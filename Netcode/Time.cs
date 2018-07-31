using System;

namespace OpenEQ.Netcode {
	class Time {
		static long StartTicks = NowTicks;
		static long NowTicks => DateTime.Now.Ticks;
		public static float Now => (NowTicks - StartTicks) / 10000000f;
	}
}