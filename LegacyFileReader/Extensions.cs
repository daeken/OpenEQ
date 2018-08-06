using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using MoreLinq.Extensions;
using OpenEQ.Common;

namespace OpenEQ.LegacyFileReader {
	public static class Extensions {
		public static Reference<T> ReadRef<T>(this BinaryReader br, Wld wld) where T : class => new Reference<T>(wld, br.ReadInt32());

		public static bool HasBit(this uint value, int bit) => (value & (1 << bit)) != 0;
	}
}