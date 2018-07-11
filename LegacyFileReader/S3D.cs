using System;
using System.Collections;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.IO;
using System.IO.Compression;
using System.Linq;
using System.Text;

namespace OpenEQ.LegacyFileReader {
	public class S3D : IEnumerable<string> {
		readonly Stream Fp;
		readonly BinaryReader Br;

		readonly Dictionary<string, (uint Offset, uint Size)> Files = new Dictionary<string, (uint Offset, uint Size)>();
		
		public S3D(Stream fp) {
			Fp = fp;
			Br = new BinaryReader(fp);

			var offset = Br.ReadUInt32();
			Debug.Assert(Br.ReadUInt32() == 0x20534650);

			fp.Position = offset;
			var chunks = Enumerable.Range(0, Br.ReadInt32()).Select(
				_ => (Crc: Br.ReadUInt32(), Offset: Br.ReadUInt32(), Size: Br.ReadUInt32())
			).ToList();

			var dchunk = chunks.First(x => x.Crc == 0x61580AC9);
			chunks = chunks.Where(x => x.Crc != 0x61580AC9).OrderBy(x => x.Offset).ToList();

			var dir = DecompressChunk(dchunk.Offset, dchunk.Size);
			using(var dms = new MemoryStream(dir)) {
				using(var dbr = new BinaryReader(dms)) {
					var fileCount = dbr.ReadUInt32();
					Debug.Assert(fileCount == chunks.Count);
					for(var i = 0; i < fileCount; ++i) {
						var str = Encoding.ASCII.GetString(dbr.ReadBytes(dbr.ReadInt32())).TrimEnd('\0');
						Files[str.ToLower()] = (chunks[i].Offset, chunks[i].Size);
					}
				}
			}
		}

		byte[] DecompressChunk(uint offset, uint tsize) {
			Fp.Position = offset;
			var arr = new byte[tsize];
			var off = 0;
			while(off < tsize) {
				var dlen = Br.ReadUInt32();
				var ilen = Br.ReadInt32();
				var cpos = Fp.Position += 2;
				using(var gzs = new DeflateStream(Fp, CompressionMode.Decompress, leaveOpen: true)) {
					gzs.Read(arr, off, ilen);
					off += ilen;
				}

				Fp.Position = cpos + dlen - 2;
			}
			return arr;
		}
		
		public byte[] this[string fn] => DecompressChunk(Files[fn].Offset, Files[fn].Size);
		public Stream Open(string fn) => new MemoryStream(this[fn], writable: false);
		
		public IEnumerator<string> GetEnumerator() => Files.Keys.GetEnumerator();
		IEnumerator IEnumerable.GetEnumerator() => GetEnumerator();

		public IEnumerable<(string, byte[])> GetAllFiles() => Files.Keys.Select(fn => (fn, this[fn]));
	}
}