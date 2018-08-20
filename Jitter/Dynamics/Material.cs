namespace Jitter.Dynamics {
	// TODO: Check values, Documenation
	// Maybe some default materials, aka Material.Soft?
	public class Material {
		internal float kineticFriction = 0.3f;
		internal float restitution;
		internal float staticFriction = 0.6f;

		public float Restitution {
			get => restitution;
			set => restitution = value;
		}

		public float StaticFriction {
			get => staticFriction;
			set => staticFriction = value;
		}

		public float KineticFriction {
			get => kineticFriction;
			set => kineticFriction = value;
		}
	}
}