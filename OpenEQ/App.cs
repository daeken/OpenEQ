namespace OpenEQ {
	class App {
		static void Main(string[] args) {
			var controller = Controller.Instance;
			controller.LoadZone(args[0]);
			controller.LoadCharacter("gfaydark_chr", "ORC");
			controller.Start();
		}
	}
}