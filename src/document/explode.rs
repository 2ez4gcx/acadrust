//! Document-aware INSERT explosion.
//!
//! The entity-level operation deliberately has no access to block tables or
//! object dictionaries.  This module supplies the context needed by the DWG
//! writer: block eligibility, base-point normalization, fresh identities, and
//! cloned extension-dictionary graphs.

use std::collections::HashMap;

use super::CadDocument;
use crate::entities::{EntityType, Insert};
use crate::types::{Handle, Transform};

impl CadDocument {
    /// Explode an INSERT using this document's block table and object graph.
    ///
    /// A MINSERT, xref, xref-dependent, or non-explodable definition is
    /// refused just as AutoCAD's EXPLODE command is.  The returned entities
    /// are detached from the source graph and have fresh handles suitable for
    /// insertion into this document.  The source INSERT and block contents
    /// are never modified.
    pub(crate) fn explode_insert_with_context(&mut self, insert: &Insert) -> Vec<EntityType> {
        if insert.is_minsert() {
            return Vec::new();
        }
        let Some(record) = self.block_records.get(&insert.block_name).cloned() else {
            return Vec::new();
        };
        if !record.explodable
            || record.flags.is_xref
            || record.flags.is_xref_overlay
            || record.flags.is_external
            || !record.xref_path.is_empty()
        {
            return Vec::new();
        }

        let base = self
            .get_entity(record.block_entity_handle)
            .and_then(|entity| match entity {
                EntityType::Block(block) => Some(block.base_point),
                _ => None,
            })
            .unwrap_or(record.base_point);
        let to_base = Transform::from_translation(base * -1.0);
        let source: Vec<EntityType> = record
            .entity_handles
            .iter()
            .filter_map(|handle| self.get_entity(*handle).cloned())
            .map(|mut entity| {
                entity.as_entity_mut().apply_transform(&to_base);
                entity
            })
            .collect();

        let transform = insert.get_transform();
        let mut parts = insert.explode_with_transform(&source, &transform, false);
        self.detach_exploded_parts(&mut parts, insert.common.owner_handle);
        parts
    }

    /// Assign fresh entity and compound-child identities and clone each
    /// entity's extension dictionary.  A source extension dictionary is a
    /// hard-owned graph; retaining its handle under a copied entity makes the
    /// original and exploded entity race for the same owner at DWG save time.
    fn detach_exploded_parts(&mut self, parts: &mut [EntityType], owner: Handle) {
        for part in parts {
            let old = part.common().handle;
            let fresh = self.allocate_handle();
            if !old.is_null() {
                self.clone_extension_graph(old, fresh);
                part.common_mut().xdictionary_handle = self.extension_dictionary_handle(fresh);
            }
            part.common_mut().handle = fresh;
            part.common_mut().owner_handle = owner;
            part.common_mut().raw_record = None;
            part.common_mut().reactors.clear();
            if old.is_null() {
                part.common_mut().xdictionary_handle = None;
            }

            if let EntityType::Insert(nested) = part {
                for attribute in &mut nested.attributes {
                    let old_attribute = attribute.common.handle;
                    let fresh_attribute = self.allocate_handle();
                    if !old_attribute.is_null() {
                        self.clone_extension_graph(old_attribute, fresh_attribute);
                        attribute.common.xdictionary_handle =
                            self.extension_dictionary_handle(fresh_attribute);
                    }
                    attribute.common.handle = fresh_attribute;
                    attribute.common.owner_handle = fresh;
                    attribute.common.raw_record = None;
                    attribute.common.reactors.clear();
                    if old_attribute.is_null() {
                        attribute.common.xdictionary_handle = None;
                    }
                }
                nested.view_rep_handle = None;
                nested.seqend_handle = nested.seqend_handle.map(|_| self.allocate_handle());
            }
        }
    }

    /// Clone an entity's extension dictionary and the dictionary/XRECORD graph
    /// it owns.  The graph is intentionally limited to object kinds whose
    /// handle relationships are represented by the public object model; an
    /// opaque object remains attached to its source and is dropped from the
    /// detached entity rather than producing a dangling reference.
    fn clone_extension_graph(&mut self, source_owner: Handle, target_owner: Handle) {
        // The block graph remapper already handles every typed object family,
        // ownership chain, XDATA handle and storage provenance.  Reuse it for
        // an entity extension dictionary as well; a source INSERT's xdict is
        // the same hard-owned graph shape as a block record's xdict.
        if let Some(graph) = self.export_block_graph(source_owner) {
            self.import_block_graph(&graph, target_owner, &HashMap::new());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::{Block, Line};
    use crate::objects::ObjectType;
    use crate::tables::BlockRecord;
    use crate::types::Vector3;

    fn document_with_line() -> (CadDocument, Insert, Handle, Handle) {
        let mut document = CadDocument::new();
        let mut record = BlockRecord::new("B");
        record.handle = document.allocate_handle();
        let owner = record.handle;
        document.block_records.add(record).unwrap();

        let mut block = Block::new("B", Vector3::ZERO);
        block.common.owner_handle = owner;
        let block_handle = document.add_entity(EntityType::Block(block)).unwrap();
        document
            .block_records
            .get_mut("B")
            .unwrap()
            .block_entity_handle = block_handle;

        let mut line = Line::from_points(Vector3::ZERO, Vector3::new(1.0, 0.0, 0.0));
        line.common.owner_handle = owner;
        let line_handle = document.add_entity(EntityType::Line(line)).unwrap();

        let mut insert = Insert::new("B", Vector3::new(10.0, 20.0, 0.0));
        insert.common.owner_handle = document.header.model_space_block_handle;
        (document, insert, owner, line_handle)
    }

    #[test]
    fn context_explode_refuses_xrefs_and_minsert() {
        let (mut document, mut insert, _, _) = document_with_line();
        document.block_records.get_mut("B").unwrap().flags.is_xref = true;
        assert!(document
            .explode_entity(&EntityType::Insert(insert.clone()))
            .is_empty());
        document.block_records.get_mut("B").unwrap().flags.is_xref = false;
        insert = insert.with_array(2, 1, 5.0, 0.0);
        assert!(document
            .explode_entity(&EntityType::Insert(insert))
            .is_empty());
    }

    #[test]
    fn context_explode_assigns_fresh_owner_and_handle() {
        let (mut document, insert, _owner, source_handle) = document_with_line();
        let parts = document.explode_entity(&EntityType::Insert(insert));
        assert_eq!(parts.len(), 1);
        let line = match &parts[0] {
            EntityType::Line(value) => value,
            _ => panic!("expected line"),
        };
        assert!(line.common.handle.is_valid());
        assert_ne!(line.common.handle, source_handle);
        assert_eq!(
            line.common.owner_handle,
            document.header.model_space_block_handle
        );
        assert_eq!(line.start, Vector3::new(10.0, 20.0, 0.0));
    }

    #[test]
    fn context_explode_clones_entity_extension_dictionary() {
        let (mut document, insert, _, source_handle) = document_with_line();
        let source_dict = document.ensure_extension_dictionary(source_handle);
        let source_record = document.ensure_xrecord(source_handle, "EXPLODE_TEST");
        assert_eq!(
            document.extension_dictionary_handle(source_handle),
            Some(source_dict)
        );
        assert!(document.objects.contains_key(&source_record));

        let parts = document.explode_entity(&EntityType::Insert(insert));
        let line = match &parts[0] {
            EntityType::Line(value) => value,
            _ => panic!("expected line"),
        };
        let cloned_dict = line.common.xdictionary_handle.expect("cloned dictionary");
        assert_ne!(cloned_dict, source_dict);
        assert!(matches!(
            document.objects.get(&cloned_dict),
            Some(ObjectType::Dictionary(_))
        ));
        assert_eq!(
            document.extension_dictionary_handle(line.common.handle),
            Some(cloned_dict)
        );
    }

    #[test]
    fn context_explode_remaps_nested_insert_children() {
        let (mut document, mut insert, _, _) = document_with_line();
        let owner = document.block_records.get("B").unwrap().handle;
        let mut nested = Insert::new("CHILD", Vector3::new(1.0, 2.0, 0.0));
        nested.common.handle = Handle::new(0x1200);
        nested.common.owner_handle = owner;
        nested.seqend_handle = Some(Handle::new(0x1202));
        let mut attribute = crate::entities::AttributeEntity::new("TAG".into(), "value".into());
        attribute.common.handle = Handle::new(0x1201);
        attribute.common.owner_handle = nested.common.handle;
        nested.attributes.push(attribute);
        document.add_entity(EntityType::Insert(nested)).unwrap();

        // The helper's block member list now contains the line and nested
        // INSERT.  The source nested record remains untouched after EXPLODE.
        insert.common.owner_handle = document.header.model_space_block_handle;
        let parts = document.explode_entity(&EntityType::Insert(insert));
        let output = parts
            .iter()
            .find_map(|part| match part {
                EntityType::Insert(value) => Some(value),
                _ => None,
            })
            .expect("nested insert output");
        assert!(output.common.handle.is_valid());
        assert_ne!(output.common.handle, Handle::new(0x1200));
        assert_eq!(output.attributes.len(), 1);
        assert!(output.attributes[0].common.handle.is_valid());
        assert_ne!(output.attributes[0].common.handle, Handle::new(0x1201));
        assert_eq!(
            output.attributes[0].common.owner_handle,
            output.common.handle
        );
        assert_ne!(output.seqend_handle, Some(Handle::new(0x1202)));
    }
}
