namespace Physics {
	public class World {
		public static World Instance;
		
		public readonly PhysicalPlayer Player;
		public readonly Octree FixedOctree;

		public World(PhysicalPlayer player, Octree fixedOctree) {
			Player = player;
			FixedOctree = fixedOctree;
		}

		public void Update(float timeStep) {
			Player.Update(timeStep);
		}
	}
}