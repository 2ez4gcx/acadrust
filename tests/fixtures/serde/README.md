# Legacy Serde fixture

`retained_fields_legacy.json` is frozen JSON output from unmodified upstream
`cc17ec1360531325deaf0a492afbf23738bcefbd` (acadrust 0.6.4), before the thirteen
reference/provenance fields were included by Serde. It contains EntityCommon,
Spline, Helix with its embedded Spline, and AssociativeObject.

The objects use their public constructors. Common data has layer `LegacyLayer`
and `full_visual_style_handle = Some(Handle::new(42))`; the Spline and both
Helix common-data records use that common data. The associative object uses
`AssociativeObject::new("ASSOC", "AcDbAssoc")`. The file was produced using
`serde_json::to_string_pretty` on those four objects, followed by a newline.

Keep this old input literal: do not regenerate it using the updated serializers.
The regression checks every newly included field is absent and that reading the
old input retains unrelated values while supplying documented absence defaults.
