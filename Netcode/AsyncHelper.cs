using System;
using System.Threading.Tasks;
using static System.Console;

namespace OpenEQ.Netcode {
	public class AsyncHelper {
		public static void Run(Action func, bool longRunning = false) {
			var tlst = Environment.StackTrace;
			Task.Factory.StartNew(() => {
				try {
					func();
				} catch(Exception e) {
					WriteLine($"Async task threw exception ${e}");
					WriteLine(e.StackTrace);
					WriteLine("Outer stack trace:");
					WriteLine(tlst);
					System.Environment.Exit(0);
				}
			}, longRunning ? TaskCreationOptions.LongRunning : TaskCreationOptions.None);
		}
	}
}