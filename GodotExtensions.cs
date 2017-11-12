using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using Godot;
using System.Reflection;

namespace OpenEQ {
	public static class GodotExtensions {
		// Shamelessly ripped from https://stackoverflow.com/questions/2119441/check-if-types-are-castable-subclasses
		public static bool IsCastableTo(this Type from, Type to, bool implicitly = false) {
			return to.IsAssignableFrom(from) || from.HasCastDefined(to, implicitly);
		}

		static bool HasCastDefined(this Type from, Type to, bool implicitly) {
			if((from.IsPrimitive || from.IsEnum) && (to.IsPrimitive || to.IsEnum)) {
				if(!implicitly)
					return from == to || (from != typeof(Boolean) && to != typeof(Boolean));

				Type[][] typeHierarchy = {
					new Type[] { typeof(Byte),  typeof(SByte), typeof(Char) },
					new Type[] { typeof(Int16), typeof(UInt16) },
					new Type[] { typeof(Int32), typeof(UInt32) },
					new Type[] { typeof(Int64), typeof(UInt64) },
					new Type[] { typeof(Single) },
					new Type[] { typeof(Double) }
				};
				var lowerTypes = Enumerable.Empty<Type>();
				foreach(var types in typeHierarchy) {
					if(types.Any(t => t == to))
						return lowerTypes.Any(t => t == from);
					lowerTypes = lowerTypes.Concat(types);
				}

				return false;   // IntPtr, UIntPtr, Enum, Boolean
			}
			return IsCastDefined(to, m => m.GetParameters()[0].ParameterType, _ => from, implicitly, false)
				|| IsCastDefined(from, _ => to, m => m.ReturnType, implicitly, true);
		}

		static bool IsCastDefined(Type type, Func<MethodInfo, Type> baseType, Func<MethodInfo, Type> derivedType, bool implicitly, bool lookInBase) {
			var bindingFlags = BindingFlags.Public | BindingFlags.Static
							| (lookInBase ? BindingFlags.FlattenHierarchy : BindingFlags.DeclaredOnly);
			return type.GetMethods(bindingFlags).Any(
				m => (m.Name == "op_Implicit" || (!implicitly && m.Name == "op_Explicit"))
					&& baseType(m).IsAssignableFrom(derivedType(m)));
		}

		public static T GetNode<T>(this Node node, NodePath path, bool nullAllowed = false, bool blindCast = false) where T : Node {
			var sub = node.GetNode(path);
			if(sub != null) {
				if(blindCast || sub.GetType().IsCastableTo(typeof(T)))
					return (T) sub;
				else
					throw new Exception($"Node {sub.GetPath()} not castable to {typeof(T)}");
			} else if(nullAllowed)
				return null;
			throw new Exception($"Could not get {path} from node '{node.GetPath()}'.");
		}

		public static T Get<T>(this Node node, string propname) {
			return (T) node.Get(propname);
		}
	}
}
