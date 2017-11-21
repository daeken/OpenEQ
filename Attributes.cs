using PostSharp.Aspects;
using PostSharp.Serialization;
using Godot;

using static System.Console;

namespace OpenEQ {
	[PSerializable]
	public sealed class ControlGetterAttribute : LocationInterceptionAspect {
		string Path;
		public ControlGetterAttribute(string path = null) {
			Path = path;
		}

		public override void OnGetValue(LocationInterceptionArgs args) {
			base.OnGetValue(args);
			if(args.Value == null) {
				var obj = ((Node) args.Instance).GetNode(Path ?? args.LocationName);
				args.SetNewValue(obj);
				args.Value = obj;
			}
		}
	}
}
