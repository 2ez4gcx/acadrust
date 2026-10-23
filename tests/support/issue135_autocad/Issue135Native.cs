using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using Autodesk.AutoCAD.ApplicationServices;
using Autodesk.AutoCAD.DatabaseServices;
using Autodesk.AutoCAD.Geometry;
using Autodesk.AutoCAD.Runtime;

public class Issue135Native
{
    private static string Output => Environment.GetEnvironmentVariable("ISSUE135_OUTPUT") ?? Directory.GetCurrentDirectory();

    [CommandMethod("ISSUE135GENERATE")]
    public void Generate()
    {
        var doc = Application.DocumentManager.MdiActiveDocument;
        var db = doc.Database;
        Directory.CreateDirectory(Output);
        using (var tr = db.TransactionManager.StartTransaction())
        {
            var model = (BlockTableRecord)tr.GetObject(SymbolUtilityServices.GetBlockModelSpaceId(db), OpenMode.ForWrite);
            var networkId = AssocNetwork.GetInstanceFromObject(model.ObjectId, true, true, "");
            var network = (AssocNetwork)tr.GetObject(networkId, OpenMode.ForWrite);
            Action<string, Entity[], GeometricalConstraint.ConstraintType[]> scene = (name, entities, constraints) =>
            {
                doc.Editor.WriteMessage("\nISSUE135_BEGIN " + name);
                var group = new Assoc2dConstraintGroup(new Plane(Point3d.Origin, Vector3d.ZAxis));
                db.AddDBObject(group);
                tr.AddNewlyCreatedDBObject(group, true);
                network.AddAction(group.ObjectId, true);
                var geometries = new List<ConstrainedGeometry>();
                foreach (var entity in entities)
                {
                    model.AppendEntity(entity);
                    tr.AddNewlyCreatedDBObject(entity, true);
                    var path = new FullSubentityPath(new[] { entity.ObjectId }, new SubentityId(SubentityType.Edge, new IntPtr(1)));
                    geometries.Add(group.AddConstrainedGeometry(path));
                }
                foreach (var kind in constraints)
                {
                    group.AddGeometricalConstraint(kind, geometries.ToArray());
                }
                doc.Editor.WriteMessage("\nISSUE135_SCENE {0} group={1} geometries={2} constraints={3}", name, group.Handle, group.ConstrainedGeometries.Length, group.Constraints.Length);
            };
            scene("horizontal", new Entity[] { Line(0, 0, 20, 0) }, new[] { GeometricalConstraint.ConstraintType.Horizontal });
            scene("vertical", new Entity[] { Line(30, 0, 30, 20) }, new[] { GeometricalConstraint.ConstraintType.Vertical });
            scene("parallel", new Entity[] { Line(40, 0, 60, 0), Line(40, 10, 60, 10) }, new[] { GeometricalConstraint.ConstraintType.Parallel });
            scene("perpendicular", new Entity[] { Line(70, 0, 90, 0), Line(80, -10, 80, 10) }, new[] { GeometricalConstraint.ConstraintType.Perpendicular });
            scene("collinear", new Entity[] { Line(100, 0, 110, 0), Line(120, 0, 130, 0) }, new[] { GeometricalConstraint.ConstraintType.Collinear });
            scene("equal_length", new Entity[] { Line(140, 0, 160, 0), Line(140, 10, 160, 10) }, new[] { GeometricalConstraint.ConstraintType.EqualLength });
            scene("concentric", new Entity[] { Circle(0, 50, 10), Circle(0, 50, 15) }, new[] { GeometricalConstraint.ConstraintType.Concentric });
            scene("equal_radius", new Entity[] { Circle(40, 50, 10), Circle(70, 50, 10) }, new[] { GeometricalConstraint.ConstraintType.EqualRadius });
            scene("tangent", new Entity[] { Circle(100, 50, 10), Line(80, 40, 120, 40) }, new[] { GeometricalConstraint.ConstraintType.Tangent });
            scene("arc", new Entity[] { new Arc(new Point3d(140, 50, 0), 10, 0, Math.PI) }, new[] { GeometricalConstraint.ConstraintType.Fix });
            scene("ellipse", new Entity[] { new Ellipse(new Point3d(0, 100, 0), Vector3d.ZAxis, new Vector3d(15, 0, 0), 0.5, 0, 2 * Math.PI) }, new[] { GeometricalConstraint.ConstraintType.Fix });
            scene("elliptical_arc", new Entity[] { new Ellipse(new Point3d(40, 100, 0), Vector3d.ZAxis, new Vector3d(15, 0, 0), 0.5, 0, Math.PI) }, new[] { GeometricalConstraint.ConstraintType.Fix });
            scene("spline", new Entity[] { new Spline(new Point3dCollection(new[] { new Point3d(70, 100, 0), new Point3d(80, 110, 0), new Point3d(90, 95, 0), new Point3d(100, 100, 0) }), 3, 0.0) }, new[] { GeometricalConstraint.ConstraintType.Fix });
            tr.Commit();
        }
        AssocManager.EvaluateTopLevelNetwork(db, null, 0);
        Inspect();
        foreach (var pair in new[] { ("2010", DwgVersion.AC1024), ("2013", DwgVersion.AC1027), ("2018", DwgVersion.AC1032) })
        {
            db.SaveAs(Path.Combine(Output, "native_" + pair.Item1 + ".dwg"), pair.Item2);
            db.DxfOut(Path.Combine(Output, "native_" + pair.Item1 + ".dxf"), 16, pair.Item2);
        }
    }

    private static Line Line(double x1, double y1, double x2, double y2) => new Line(new Point3d(x1, y1, 0), new Point3d(x2, y2, 0));
    private static Circle Circle(double x, double y, double radius) => new Circle(new Point3d(x, y, 0), Vector3d.ZAxis, radius);

    [CommandMethod("ISSUE135VALIDATE")]
    public void Validate()
    {
        var doc = Application.DocumentManager.MdiActiveDocument;
        var tag = Environment.GetEnvironmentVariable("ISSUE135_TAG") ?? "validation";
        Inspect();
        var before = File.ReadAllLines(Path.Combine(Output, tag + ".inventory.txt"));
        AssocManager.EvaluateTopLevelNetwork(doc.Database, null, 0);
        Inspect();
        var after = File.ReadAllLines(Path.Combine(Output, tag + ".inventory.txt"));
        if (!before.SequenceEqual(after)) throw new InvalidOperationException("Constraint inventory changed during evaluation.");
        doc.Database.SaveAs(Path.Combine(Output, tag + "_autocad.dwg"), DwgVersion.AC1032);
        doc.Database.DxfOut(Path.Combine(Output, tag + "_autocad.dxf"), 16, DwgVersion.AC1032);
        if (Environment.GetEnvironmentVariable("ISSUE135_FUNCTIONAL") == "1") CheckHorizontalConstraint();
        doc.Editor.WriteMessage("\nISSUE135_EVALUATE_UNCHANGED saved={0}_autocad.dwg", tag);
    }

    private static void CheckHorizontalConstraint()
    {
        var doc = Application.DocumentManager.MdiActiveDocument;
        var db = doc.Database;
        ObjectId target = ObjectId.Null;
        using (var tr = db.TransactionManager.StartTransaction())
        {
            var model = (BlockTableRecord)tr.GetObject(SymbolUtilityServices.GetBlockModelSpaceId(db), OpenMode.ForRead);
            foreach (ObjectId id in model)
            {
                if (tr.GetObject(id, OpenMode.ForRead) is Line line &&
                    line.StartPoint.DistanceTo(Point3d.Origin) < 1e-8 &&
                    line.EndPoint.DistanceTo(new Point3d(20, 0, 0)) < 1e-8)
                {
                    target = id;
                    line.UpgradeOpen();
                    line.EndPoint = new Point3d(20, 5, 0);
                    break;
                }
            }
            if (target.IsNull) throw new InvalidOperationException("Native fixture horizontal line missing.");
            tr.Commit();
        }
        AssocManager.EvaluateTopLevelNetwork(db, null, 0);
        using (var tr = db.TransactionManager.StartTransaction())
        {
            var line = (Line)tr.GetObject(target, OpenMode.ForRead);
            if (Math.Abs(line.EndPoint.Y - line.StartPoint.Y) > 1e-8)
                throw new InvalidOperationException("Horizontal constraint did not correct the edited endpoint.");
            tr.Commit();
        }
        doc.Editor.WriteMessage("\nISSUE135_FUNCTIONAL_PASS horizontal endpoint edit corrected");
    }

    [CommandMethod("ISSUE135INSPECT")]
    public void Inspect()
    {
        var doc = Application.DocumentManager.MdiActiveDocument;
        var db = doc.Database;
        var groups = new List<Assoc2dConstraintGroup>();
        var lines = new List<string>();
        using (var tr = db.TransactionManager.StartTransaction())
        {
            for (long h = 1; h < db.Handseed.Value; h++)
            {
                if (!db.TryGetObjectId(new Handle(h), out var id) || id.IsErased) continue;
                if (tr.GetObject(id, OpenMode.ForRead) is not Assoc2dConstraintGroup group) continue;
                groups.Add(group);
                lines.Add($"GROUP {group.Handle} geometries={group.ConstrainedGeometries.Length} constraints={group.Constraints.Length} dof={group.GetDOF} status={group.SolutionStatus(false)}");
                lines.AddRange(group.ConstrainedGeometries.Select(g => $"GEOMETRY {g.NodeId} {g.GetType().Name}"));
                lines.AddRange(group.Constraints.Select(c => $"CONSTRAINT {c.NodeId} {c.GetType().Name} active={c.IsActive} implied={c.IsImplied}"));
            }
            lines.Add($"ISSUE135_SUMMARY groups={groups.Count} geometries={groups.Sum(g => g.ConstrainedGeometries.Length)} constraints={groups.Sum(g => g.Constraints.Length)}");
            tr.Commit();
        }
        foreach (var line in lines) doc.Editor.WriteMessage("\n" + line);
        var tag = Environment.GetEnvironmentVariable("ISSUE135_TAG") ?? "native";
        File.WriteAllLines(Path.Combine(Output, tag + ".inventory.txt"), lines);
    }
}
