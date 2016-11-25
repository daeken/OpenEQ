using System.Diagnostics;

namespace OpenEQ {
    public static class Time {
        static Stopwatch watch; 
        public static float Now {
            get {
                return (float) watch.Elapsed.TotalSeconds;
            }
        }

        static Time() {
            watch = new Stopwatch();
            watch.Start();
        }
    }
}