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

		public static IEnumerable<int> Times(this int count) => Enumerable.Range(0, count);
		public static IEnumerable<int> Times(this uint count) => ((int) count).Times();
		public static IEnumerable<int> Times(this ushort count) => ((int) count).Times();

		public static IEnumerable<T> Times<T>(this int count, Func<T> func) => count.Times().Select(x => func());
		public static IEnumerable<T> Times<T>(this uint count, Func<T> func) => ((int) count).Times(func);
		public static IEnumerable<T> Times<T>(this ushort count, Func<T> func) => ((int) count).Times(func);

		public static IEnumerable<T> Times<T>(this int count, Func<int, T> func) => count.Times().Select(func);
		public static IEnumerable<T> Times<T>(this uint count, Func<int, T> func) => count.Times().Select(func);
		public static IEnumerable<T> Times<T>(this ushort count, Func<int, T> func) => count.Times().Select(func);

		public static void Times(this int count, Action func) => count.Times().ForEach(_ => func());
		public static void Times(this uint count, Action func) => count.Times().ForEach(_ => func());
		public static void Times(this ushort count, Action func) => count.Times().ForEach(_ => func());

		public static void Times(this int count, Action<int> func) => count.Times().ForEach(func);
		public static void Times(this uint count, Action<int> func) => count.Times().ForEach(func);
		public static void Times(this ushort count, Action<int> func) => count.Times().ForEach(func);
	}
}