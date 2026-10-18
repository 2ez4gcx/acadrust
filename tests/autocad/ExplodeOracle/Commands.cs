using Autodesk.AutoCAD.ApplicationServices.Core;
using Autodesk.AutoCAD.DatabaseServices;
using Autodesk.AutoCAD.Geometry;
using Autodesk.AutoCAD.Runtime;
using System.IO;

public sealed class Commands
{
    private static readonly Point3dCollection Quad = new(new[]
    {
        new Point3d(0, 0, 0), new Point3d(10, 0, 0),
        new Point3d(10, 10, 0), new Point3d(0, 10, 0)
    });

    private static ObjectId Add(Entity entity)
    {
        var database = Application.DocumentManager.MdiActiveDocument.Database;
        using var transaction = database.TransactionManager.StartTransaction();
        var space = (BlockTableRecord)transaction.GetObject(database.CurrentSpaceId, OpenMode.ForWrite);
        var id = space.AppendEntity(entity);
        transaction.AddNewlyCreatedDBObject(entity, true);
        transaction.Commit();
        return id;
    }

    private static HashSet<ObjectId> Existing()
    {
        var database = Application.DocumentManager.MdiActiveDocument.Database;
        using var transaction = database.TransactionManager.StartTransaction();
        var space = (BlockTableRecord)transaction.GetObject(database.CurrentSpaceId, OpenMode.ForRead);
        return space.Cast<ObjectId>().ToHashSet();
    }

    private static Entity NonuniformAcis(bool mixed)
    {
        var database = Application.DocumentManager.MdiActiveDocument.Database;
        using var transaction = database.TransactionManager.StartTransaction();
        var blocks = (BlockTable)transaction.GetObject(database.BlockTableId, OpenMode.ForWrite);
        var block = new BlockTableRecord { Name = mixed ? "AE_ACIS_MIXED" : "AE_ACIS_ONLY" };
        var id = blocks.Add(block);
        transaction.AddNewlyCreatedDBObject(block, true);
        var solid = new Solid3d();
        solid.CreateBox(10, 10, 10);
        block.AppendEntity(solid);
        transaction.AddNewlyCreatedDBObject(solid, true);
        if (mixed)
        {
            var line = new Line(Point3d.Origin, new Point3d(20, 0, 0));
            block.AppendEntity(line);
            transaction.AddNewlyCreatedDBObject(line, true);
        }
        transaction.Commit();
        return new BlockReference(Point3d.Origin, id) { ScaleFactors = new Scale3d(2, 1, 1) };
    }

    private static Entity CustomDimension()
    {
        var database = Application.DocumentManager.MdiActiveDocument.Database;
        ObjectId styleId;
        using (var transaction = database.TransactionManager.StartTransaction())
        {
            var table = (DimStyleTable)transaction.GetObject(database.DimStyleTableId, OpenMode.ForWrite);
            const string name = "AE_CUSTOM_DIM";
            if (table.Has(name))
            {
                styleId = table[name];
            }
            else
            {
                var style = new DimStyleTableRecord
                {
                    Name = name,
                    Dimtxt = 3.0,
                    Dimasz = 1.0,
                    Dimexo = 1.0,
                    Dimexe = 2.0,
                    Dimlfac = 2.0,
                    Dimdec = 1,
                    Dimpost = "<> mm",
                    Dimgap = 0.5
                };
                styleId = table.Add(style);
                transaction.AddNewlyCreatedDBObject(style, true);
            }
            transaction.Commit();
        }
        return new AlignedDimension(
            new Point3d(0, 0, 0),
            new Point3d(10, 0, 0),
            new Point3d(0, 5, 0),
            "",
            styleId);
    }

    private static Entity CurvedRegion()
    {
        var circle = new Circle(Point3d.Origin, Vector3d.ZAxis, 5.0);
        var curves = new DBObjectCollection { circle };
        var regions = Region.CreateFromCurves(curves);
        circle.Dispose();
        return (Region)regions[0];
    }

    private static Entity CurvedSolid()
    {
        var solid = new Solid3d();
        solid.CreateFrustum(10.0, 5.0, 5.0, 5.0);
        return solid;
    }

    private static void Run(StreamWriter output, string name, Func<Entity> create)
    {
        var editor = Application.DocumentManager.MdiActiveDocument.Editor;
        try
        {
            var before = Existing();
            var source = Add(create());
            editor.Command("_.EXPLODE", source, "");
            using var transaction = source.Database.TransactionManager.StartTransaction();
            var results = Existing().Except(before).Where(id => id != source).ToArray();
            var types = results.Select(id => ((Entity)transaction.GetObject(id, OpenMode.ForRead)).GetRXClass().DxfName);
            output.WriteLine($"{name}|SOURCE_SURVIVES|{(!source.IsErased ? "YES" : "NO")}");
            output.WriteLine($"{name}|RESULT_TYPES|{string.Join(",", types)}");
            foreach (var id in results)
            {
                if (transaction.GetObject(id, OpenMode.ForRead) is BlockReference reference)
                {
                    var block = (BlockTableRecord)transaction.GetObject(reference.BlockTableRecord, OpenMode.ForRead);
                    output.WriteLine($"{name}|RESULT_BLOCK|{block.Name}");
                }
            }
            editor.WriteMessage($"\nAE_EXTRA|{name}|{(!source.IsErased ? "SURVIVES" : "ERASED")}|{results.Length}\n");
        }
        catch (System.Exception exception)
        {
            output.WriteLine($"{name}|ERROR|{exception.GetType().Name}|{exception.Message}");
            editor.WriteMessage($"\nAE_EXTRA|{name}|ERROR|{exception.Message}\n");
        }
        output.Flush();
    }

    [CommandMethod("AE_EXTRA_ORACLE", CommandFlags.Modal)]
    public static void RunExtra()
    {
        string root = Environment.GetEnvironmentVariable("ACADRUST_ORACLE_ROOT")
            ?? throw new InvalidOperationException("ACADRUST_ORACLE_ROOT is required.");
        using var output = new StreamWriter(Path.Combine(root, "target/explode-oracle/managed-runtime.txt"));
        Run(output, "TABLE", () =>
        {
            var table = new Table { Position = Point3d.Origin };
            table.SetSize(2, 2);
            table.SetRowHeight(3);
            table.SetColumnWidth(10);
            table.Cells[0, 0].TextString = "Title";
            table.Cells[1, 0].TextString = "A";
            table.Cells[1, 1].TextString = "B";
            table.GenerateLayout();
            return table;
        });
        Run(output, "MESH_QUAD", () =>
        {
            var mesh = new SubDMesh();
            mesh.SetSubDMesh(Quad, new Int32Collection(new[] { 4, 0, 1, 2, 3 }), 0);
            return mesh;
        });
        Run(output, "MLEADER_MTEXT", () =>
        {
            var leader = new MLeader { ContentType = ContentType.MTextContent };
            using var text = new MText { Contents = "Example", Location = new Point3d(15, 10, 0), TextHeight = 2.5 };
            leader.MText = text;
            int index = leader.AddLeaderLine(new Point3d(0, 0, 0));
            leader.AddLastVertex(index, new Point3d(10, 10, 0));
            return leader;
        });
        Run(output, "TOLERANCE", () => new FeatureControlFrame
        {
            Text = "{\\Fgdt;p}%%v0.5", Location = Point3d.Origin
        });
        Run(output, "WIPEOUT", () =>
        {
            var wipeout = new Wipeout();
            wipeout.SetFrom(new Point2dCollection(new[]
            {
                new Point2d(0, 0), new Point2d(10, 0), new Point2d(10, 10),
                new Point2d(0, 10), new Point2d(0, 0)
            }), Vector3d.ZAxis);
            return wipeout;
        });
        Run(output, "INSERT_NONUNIFORM_ACIS_ONLY", () => NonuniformAcis(false));
        Run(output, "INSERT_NONUNIFORM_ACIS_MIXED", () => NonuniformAcis(true));
        Run(output, "DIM_ALIGNED_CUSTOM_STYLE", CustomDimension);
        Run(output, "REGION_CIRCLE", CurvedRegion);
        Run(output, "3DSOLID_CYLINDER", CurvedSolid);
        Application.DocumentManager.MdiActiveDocument.Editor.WriteMessage("\nAE_EXTRA|DONE\n");
        output.WriteLine("DONE");
    }
}
