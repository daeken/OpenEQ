using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using OpenEQ.FileConverter.Entities;
using OpenEQ.FileConverter.Extensions;

namespace OpenEQ.FileConverter.Wld
{
    public class WldConverter
    {
        public int FlagNormal = 0;
        public int FlagMasked = 1 << 0;
        public int FlagTranslucent = 1 << 1;
        public int FlagTransparent = 1 << 2;

        public WorldHeader Header;
        public Dictionary<int, object> frags;
        public Dictionary<uint, List<object>> byType;
        public Dictionary<string, object> names;
        public bool baked;

        private string StringTable;
        private IDictionary<string, byte[]> s3d;

        public WldConverter()
        {
            byType = new Dictionary<uint, List<object>>();
            frags = new Dictionary<int, object>();
            names = new Dictionary<string, object>();
        }

        public void Convert(byte[] wldData, IDictionary<string, byte[]> objectFiles)
        {
            s3d = objectFiles;

            using (var input = new BinaryReader(new MemoryStream(wldData)))
            {
                Header = new WorldHeader(input);

                if (0x54503D02 != Header.magic)
                    throw new FormatException("Expected magic (0x54503D02) not found.");

                // Get the string table.
                StringTable = input.ReadBytes(Header.stringHashSize).DecodeString();

                GetFragments(input);

                // Clear the values, but not the keys.
                foreach (var v in byType)
                {
                    v.Value.Clear();
                }

                var nfrags = new Dictionary<int, object>();
                var nnames = new Dictionary<string, object>();

                foreach (var frag in frags.Values.OfType<Tuple<int, string, uint, object>>())
                {
                    nfrags[frag.Item1] = nnames[frag.Item2] = frag.Item4;
                    byType[frag.Item3].Add(frag.Item4);
                }

                frags = nfrags;
                names = nnames;
                baked = true;

                //Console.WriteLine(
                //    $"fragtypes ({byType.Keys.Count}): {byType.Aggregate("", (current, v) => current + $",'{v.Key:X2}'").TrimStart(',')}");
            }
        }

        public void ConvertObjects(Zone zone)
        {
            ConvertMeshFrag(zone);

            // This one needs a fully built list of object names in the zone.
            ConvertObjLocFrag(zone);
        }

        public void ConvertLights(Zone zone)
        {
            // No conversion of lights?
        }

        public void ConvertZone(Zone zone)
        {
            if (!byType.ContainsKey(54))
                return;

            foreach (FragMesh frag in byType[54])
            {
                var vbuf = new VertexBuffer(frag);

                var off = 0;
                foreach (var polytex in frag.Polytex)
                {
                    var count = polytex[0];
                    var index = polytex[1];

                    if (frag.Textures.Length > 1)
                    {
                        throw new IndexOutOfRangeException(
                            "WldConverter.ConvertObjects -- frag.Textures.Length > 1");
                    }

                    var texnames = ((FragRef[])frag.Textures[0].Value)[index].Resolve().OfType<string>().ToList();
                    var texFlags = ((TexRef)((FragRef[])frag.Textures[0].Value)[index].Value).SaneFlags;

                    var tmpS3DData = texnames.Select(t => s3d[t.ToLower()]).ToList();

                    // The first param was texFlags, but it was always set to 0, so I'm setting it to 0.
                    var material = new Material(texFlags, tmpS3DData);
                    var mesh = new Mesh(material, vbuf, frag.Polys.Skip(off).Take(count).ToList());
                    zone.ZoneObj.Meshes.Add(mesh);
                    off += count;
                }
            }
        }

        private void ConvertObjLocFrag(Zone zone)
        {
            if (!byType.ContainsKey(21))
                return;

            foreach (ObjectLocation frag in byType[21])
            {
                var objname = StringTable.ReadNullTerminatedString(-frag.NameOffset).Replace("_ACTORDEF", "");
                zone.AddPlaceable(objname, frag.Position, frag.Rotation, frag.Scale);
            }
        }

        private void ConvertMeshFrag(Zone zone, bool zoneConvert = false)
        {
            if (!byType.ContainsKey(54))
                return;

            foreach (FragMesh frag in byType[54])
            {
                var obj = zone.AddObject(frag._name);
                var vbuf = new VertexBuffer(frag);

                var off = 0;

                foreach (var polytex in frag.Polytex)
                {
                    var count = polytex[0];
                    var index = polytex[1];

                    if (frag.Textures.Length > 1)
                    {
                        throw new IndexOutOfRangeException(
                            "WldConverter.ConvertObjects -- frag.Textures.Length > 1");
                    }

                    var texnames = ((FragRef[])frag.Textures[0].Value)[index].Resolve().OfType<string>().ToList();
                    var texFlags = ((TexRef) ((FragRef[]) frag.Textures[0].Value)[index].Value).SaneFlags;

                    var tmpS3DData = texnames.Select(t => s3d[t.ToLower()]).ToList();

                    var material = new Material(texFlags, tmpS3DData);
                    var mesh = new Mesh(material, vbuf, frag.Polys.Skip(off).Take(count).ToList());
                    obj.Meshes.Add(mesh);
                    off += count;
                }
            }
        }

        #region Fragment Handlers

        private void GetFragments(BinaryReader input)
        {
            for (var i = 0; i < Header.fragmentCount; i++)
            {
                var fragHeader = new struct_wld_basic_frag(input);

                var name =
                    fragHeader.nameoff != 0x1000000
                        ? StringTable.ReadNullTerminatedString(-Math.Min(fragHeader.nameoff, 0))
                        : "";

                var epos = input.BaseStream.Position + fragHeader.size - 4;
                object frag = null;

                switch (fragHeader.type)
                {
                    case 3:
                    {
                        frag = FragTexName(input);
                        break;
                    }
                    case 4:
                    {
                        frag = FragTexBitInfo(input);
                        break;
                    }
                    case 5:
                    {
                        frag = FragTexUnk(input);
                        break;
                    }
                    case 16:
                    {
                        frag = FragSkelTrackSet(input, name);
                        break;
                    }
                    case 17:
                    {
                        frag = FragSkelTrackSetRef(input);
                        break;
                    }
                    case 18:
                    {
                        frag = FragSkelPierceTrack(input, name);
                        break;
                    }
                    case 19:
                    {
                        frag = FragSkelPierceTrackRef(input, name);
                        break;
                    }
                    case 20:
                    {
                        frag = FragModelRef(input, name);
                        break;
                    }
                    case 21:
                    {
                        frag = FragObjLoc(input, name);
                        break;
                    }
                    case 24:
                    {
                        frag = FragLightSource(input, name);
                        break;
                    }
                    case 28:
                    {
                        frag = FragLightSourceRef(input);
                        break;
                    }
                    case 40:
                    {
                        frag = FragLightInfo(input, name);
                        break;
                    }
                    case 42:
                    {
                        FragAmbient(input);
                        break;
                    }
                    case 45:
                    {
                        frag = FragMeshRef(input);
                        break;
                    }
                    case 48:
                    {
                        frag = FragTexRef(input, name);
                        break;
                    }
                    case 49:
                    {
                        frag = FragTexList(input);
                        break;
                    }
                    case 54:
                    {
                        frag = FragMesh(input, name);
                        break;
                    }
#if DEBUG // Only build in debug mode.  No point in a release build.
                    case 8:
                    {
                        // UNKNOWN
                        var bytes = input.ReadBytes((int) fragHeader.size);
                        break;
                    }
                    case 9:
                    {
                        // UNKNOWN
                        var bytes = input.ReadBytes((int) fragHeader.size);
                        break;
                    }
                    case 22:
                    {
                            break;
                    }
                    case 27:
                    {
                        // UNKNOWN
                        var bytes = input.ReadBytes((int) fragHeader.size);
                        break;
                    }
                    case 33:
                    {
                        // UNKNOWN
                        var bytes = input.ReadBytes((int) fragHeader.size);
                        break;
                    }
                    case 34:
                    {
                        // UNKNOWN
                        var bytes = input.ReadBytes((int) fragHeader.size);
                        break;
                    }
                    case 38:
                    {
                        // UNKNOWN
                        var bytes = input.ReadBytes((int) fragHeader.size);
                        break;
                    }
                    case 41:
                    {
                        // UNKNOWN
                        var bytes = input.ReadBytes((int) fragHeader.size);
                        break;
                    }
                    case 47:
                    {
                        // UNKNOWN
                        var bytes = input.ReadBytes((int) fragHeader.size);
                        break;
                    }
                    case 50:
                    {
                            break;
                    }
                    case 51:
                    {
                        break;
                    }
                    case 52:
                    {
                        // UNKNOWN
                        var bytes = input.ReadBytes((int) fragHeader.size);
                        break;
                    }
                    case 53:
                    {
                            break;
                    }
                    case 55:
                    {
                        // UNKNOWN
                        var bytes = input.ReadBytes((int) fragHeader.size);
                        break;
                    }
#endif
                    //case 0x35: // First fragment
                    //    break;
                    //case 0x21: // BSP Tree
                    //    break;
                    default:
                        Console.WriteLine($"Unsupported fragment type: {fragHeader.type}");
                        break;
                }

                frag = new Tuple<int, string, uint, object>(i, name, fragHeader.type, frag);
                frags[i] = frag;

                if (!string.IsNullOrEmpty(name) || fragHeader.type == 0x05)
                {
                    names[name] = frag;
                }

                // Keep a list of fragments by fragment type.
                if (!byType.ContainsKey(fragHeader.type))
                    byType[fragHeader.type] = new List<object>();

                byType[fragHeader.type].Add(frag);

                input.BaseStream.Position = epos;

            }
        }

        private FragRef[] GetFrag(int reference)
        {
            return GetFrag(new[] {reference});
        }

        private FragRef[] GetFrag(IList<int> references) // r is probably a list of ints
        {
            var refs = new FragRef[references.Count];

            for (var i = 0; i < references.Count; i++)
            {
                if (references[i] > 0)
                {
                    references[i]--;
                    //((Tuple<int, string, uint, object>)frags[references[i]]).Item4
                    if (frags.ContainsKey(references[i]))
                    {
                        refs[i] = new FragRef(references[i],
                            value: ((Tuple<int, string, uint, object>) frags[references[i]]).Item4);
                    }
                    else
                    {
                        refs[i] = new FragRef(references[i]);
                    }
                }
                else
                {
                    var name = StringTable.ReadNullTerminatedString(references[i]);

                    if (names.ContainsKey(name))
                    {
                        refs[i] = new FragRef(name: name, value: names[name]);
                    }
                    else
                    {
                        refs[i] = new FragRef(name: name);
                    }
                }
            }

            return refs;
        }

        /// <summary>
        /// Handler for fragment ID 0x03
        /// </summary>
        /// <param name="input"></param>
        /// <returns></returns>
        private static string[] FragTexName(BinaryReader input)
        {
            var size = input.ReadInt32() + 1;
            var texNames = new string[size];

            for (var i = 0; i < texNames.Length; i++)
            {
                texNames[i] = input.ReadBytes(input.ReadUInt16()).DecodeString().ToLower().TrimEnd('\0');
            }

            return texNames;
        }

        /// <summary>
        /// Handler for fragment ID 0x04
        /// </summary>
        /// <param name="input"></param>
        /// <returns></returns>
        private FragRef[] FragTexBitInfo(BinaryReader input)
        {
            var flags = input.ReadUInt32();
            var size = input.ReadUInt32();

            if (0 != (flags & (1 << 2)))
                input.ReadUInt32();
            if (0 != (flags & (1 << 3)))
                input.ReadUInt32();

            return GetFrag(input.ReadInt32(size));
        }

        /// <summary>
        /// 0x05
        /// </summary>
        /// <param name="input"></param>
        /// <returns></returns>
        private FragRef[] FragTexUnk(BinaryReader input)
        {
            return GetFrag(input.ReadInt32());
        }

        /// <summary>
        /// 0x10
        /// </summary>
        /// <param name="input"></param>
        /// <param name="name"></param>
        private SkelPierceTrackSet FragSkelTrackSet(BinaryReader input, string name)
        {
            var flags = input.ReadUInt32();
            var trackcount = input.ReadUInt32();

            // We weren't using the output.  Not sure why this is here?
            GetFrag(input.ReadInt32());

            if (0 != (flags & 1))
                input.ReadUInt32(3);

            if (0 != (flags & 2))
                input.ReadSingle();

            var trackSet = new SkelPierceTrackSet(trackcount, name);

            var allMeshes = new List<int>();
            for (var i = 0; i < trackcount; i++)
            {
                var track = new SkelPierceTrack
                {
                    _name = StringTable.ReadNullTerminatedString(-input.ReadInt32()),
                    flags = input.ReadUInt32(),
                    pierceTrack = GetFrag(input.ReadInt32())
                };

                allMeshes.Add(input.ReadInt32());
                track.NextPieces = input.ReadInt32(input.ReadUInt32());
                trackSet.Tracks[i] = track;
            }

            var meshes = new List<FragRef>();

            if (0 != (flags & 0x200))
                meshes.AddRange(GetFrag(input.ReadInt32(input.ReadUInt32())));
            else
            {
                for (var i = 0; i < allMeshes.Count(); i++)
                {
                    if (0 != allMeshes[i])
                        meshes.AddRange(GetFrag(allMeshes[i]));
                }
            }

            trackSet.Meshes = meshes;
            return trackSet;
        }

        /// <summary>
        /// 0x11
        /// </summary>
        /// <param name="input"></param>
        /// <returns></returns>
        private FragRef[] FragSkelTrackSetRef(BinaryReader input)
        {
            return GetFrag(input.ReadInt32());
        }

        /// <summary>
        /// 0x12
        /// </summary>
        /// <param name="input"></param>
        /// <returns></returns>
        private Frame[] FragSkelPierceTrack(BinaryReader input, string name)
        {
            // Skipping flags?
            input.ReadUInt32();

            //var large = (flags & 8) > 0;

            var framecount = input.ReadUInt32();

            var frames = new Frame[framecount];

            for (var i = 0; i < framecount; i++)
            {
                var rotw = (float) input.ReadInt16();
                var rotx = (float) input.ReadInt16();
                var roty = (float) input.ReadInt16();
                var rotz = (float) input.ReadInt16();

                var shiftx = (float) input.ReadInt16();
                var shifty = (float) input.ReadInt16();
                var shiftz = (float) input.ReadInt16();
                var shiftden = (float) input.ReadInt16();

                if (0 != rotw)
                {
                    rotx /= 16384F;
                    roty /= 16384F;
                    rotz /= 16384F;
                    rotw /= 16384F;
                }
                else
                {
                    rotx = roty = rotz = 0;
                    rotw = 1;
                }

                if (0 != shiftden)
                {
                    shiftx /= shiftden;
                    shifty /= shiftden;
                    shiftz /= shiftden;
                }
                else
                {
                    shiftx = shifty = shiftz = 0;
                }

                frames[i] = new Frame(name, shiftx, shifty, shiftz, -rotx, -roty, -rotz, rotw);
            }

            return frames;
        }

        /// <summary>
        /// 0x13
        /// </summary>
        /// <param name="input"></param>
        /// <param name="name"></param>
        /// <returns></returns>
        private SkelPierceTrack FragSkelPierceTrackRef(BinaryReader input, string name)
        {
            var skelpiecetrack = GetFrag(input.ReadInt32());
            var flags = input.ReadUInt32();

            if (0 != (flags & 1))
                input.ReadUInt32();

            return new SkelPierceTrack(name, skelpiecetrack);
        }

        /// <summary>
        /// Handler for 0x14
        /// </summary>
        /// <param name="input"></param>
        /// <param name="name"></param>
        private ModelRef FragModelRef(BinaryReader input, string name)
        {
            var flags = input.ReadUInt32();

            // Skip this.
            input.ReadUInt32();

            var size1 = input.ReadUInt32();
            var size2 = input.ReadUInt32();

            // Skip this.
            input.ReadUInt32();

            if (0 != (flags & 1))
                input.ReadUInt32();

            if (0 != (flags & 2))
                input.ReadUInt32();

            for (var i = 0; i < size1; i++)
            {
                var e1size = input.ReadUInt32();
                var eldata = new float[e1size];

                for (var j = 0; j < e1size; j++)
                {
                    eldata[input.ReadUInt32()] = input.ReadSingle();
                }
            }

            var frags3 = input.ReadInt32(size2);

            // A string, but it seems to be blank?  Skipping it anyway.
            input.ReadBytes(input.ReadInt32()).DecodeString();

            return new ModelRef(name, GetFrag(frags3));
        }

        /// <summary>
        /// Handler for 0x15
        /// </summary>
        /// <param name="input"></param>
        /// <param name="name"></param>
        private static ObjectLocation FragObjLoc(BinaryReader input, string name)
        {
            var nameOffset = input.ReadInt32();

            // flags, we just want to skip them.
            input.ReadUInt32();

            // Some unknown value that we're skipping.
            input.ReadUInt32();
            var pos = input.ReadSingle(3);
            var rot = input.ReadSingle(3);
            rot[0] = (float) (rot[2]/512F*360F*Math.PI/180F);
            rot[1] = (float) (rot[1]/512F*360F*Math.PI/180F);
            rot[2] = (float) (rot[0]/512F*360F*Math.PI/180F);

            var scale = input.ReadSingle(3);
            scale = scale[2] > 0.0001 ? new[] {scale[2], scale[2], scale[2]} : new[] {1F, 1F, 1F};

            // Some unknown value that we're skipping.
            input.ReadUInt32();

            // Params? that we're skipping.
            input.ReadUInt32();

            return new ObjectLocation(name, pos, rot, scale, nameOffset);
        }

        /// <summary>
        /// Handler for 0x18
        /// </summary>
        /// <param name="input"></param>
        /// <param name="name"></param>
        private static LightSource FragLightSource(BinaryReader input, string name)
        {
            var flags = input.ReadUInt32();

            // Params?  That we're skipping.
            input.ReadUInt32();

            var attenuation = 200.0F;
            float[] color;

            if (0 != (flags & (1 << 4)))
            {
                if (0 != (flags & (1 << 3)))
                {
                    attenuation = input.ReadUInt32();
                }

                // Skipping.
                input.ReadSingle();

                color = input.ReadSingle(3);
            }
            else
            {
                var params3A = input.ReadSingle();
                color = new[] {params3A, params3A, params3A};
            }

            return new LightSource {_name = name, Attenuation = attenuation, Color = color};
        }

        /// <summary>
        /// 0x1C
        /// </summary>
        /// <param name="input"></param>
        /// <returns></returns>
        private FragRef[] FragLightSourceRef(BinaryReader input)
        {
            return GetFrag(input.ReadInt32());
        }

        /// <summary>
        /// 0x28
        /// </summary>
        /// <param name="input"></param>
        /// <param name="name"></param>
        private LightInfo FragLightInfo(BinaryReader input, string name)
        {
            var lref = GetFrag(input.ReadInt32());
            var flags = input.ReadUInt32();
            var pos = input.ReadSingle(3);
            var radius = input.ReadSingle();

            return new LightInfo(name, lref, flags, pos, radius);
        }

        /// <summary>
        /// 0x2A
        /// </summary>
        /// <param name="input"></param>
        private void FragAmbient(BinaryReader input)
        {
            GetFrag(input.ReadInt32());
            input.ReadUInt32();
            input.ReadUInt32(input.ReadUInt32());
        }

        /// <summary>
        /// 0x2D
        /// </summary>
        /// <param name="input"></param>
        private FragRef[] FragMeshRef(BinaryReader input)
        {
            return GetFrag(input.ReadInt32());
        }

        /// <summary>
        /// 0x30
        /// </summary>
        /// <param name="input"></param>
        /// <param name="name"></param>
        /// <returns></returns>
        private TexRef FragTexRef(BinaryReader input, string name)
        {
            var pairflags = input.ReadUInt32();
            var flags = input.ReadUInt32();
            input.BaseStream.Position += 12;

            if (2 == (pairflags & 2))
                input.BaseStream.Position += 8;

            var saneflags = 0;

            if (flags == 0)
                saneflags = FlagTransparent;

            if ((flags & (2 | 8 | 16)) != 0)
                saneflags |= FlagMasked;

            if ((flags & (4 | 8)) != 0)
                saneflags |= FlagTranslucent;

            return new TexRef(name, saneflags, GetFrag(input.ReadInt32()));
        }

        /// <summary>
        /// 0x31
        /// </summary>
        /// <param name="input"></param>
        /// <returns></returns>
        private FragRef[] FragTexList(BinaryReader input)
        {
            input.BaseStream.Position += 4;
            return GetFrag(input.ReadInt32(input.ReadUInt32()));
        }

        /// <summary>
        /// 0x36
        /// </summary>
        /// <param name="input"></param>
        /// <param name="name"></param>
        private FragMesh FragMesh(BinaryReader input, string name)
        {
            // Flags?  That we're skipping.
            input.ReadUInt32();

            var tlistref = input.ReadInt32();

            // Skip whatever this is.
            input.ReadUInt32();

            return new FragMesh(input, Header.IsOldVersion, GetFrag(tlistref), name);
        }

        #endregion
    }
}