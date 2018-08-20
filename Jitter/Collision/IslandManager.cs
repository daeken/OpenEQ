using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Diagnostics;
using Jitter.Dynamics;
using Jitter.Dynamics.Constraints;

namespace Jitter.Collision {
    /// <summary>
    ///     bodies have: connections - bodies they are connected with (via constraints or arbiters)
    ///     arbiters    - all arbiters they are involved
    ///     constraints - all constraints they are involved
    ///     static bodies dont have any connections. Think of the islands as a graph:
    ///     nodes are the bodies, and edges are the connections
    /// </summary>
    internal class IslandManager : ReadOnlyCollection<CollisionIsland> {
		public static ResourcePool<CollisionIsland> Pool = new ResourcePool<CollisionIsland>();

		readonly List<CollisionIsland> islands;


		readonly Queue<RigidBody> leftSearchQueue = new Queue<RigidBody>();
		readonly Queue<RigidBody> rightSearchQueue = new Queue<RigidBody>();
		readonly Stack<Arbiter> rmStackArb = new Stack<Arbiter>();
		readonly Stack<Constraint> rmStackCstr = new Stack<Constraint>();

		readonly Stack<RigidBody> rmStackRb = new Stack<RigidBody>();

		readonly List<RigidBody> visitedBodiesLeft = new List<RigidBody>();
		readonly List<RigidBody> visitedBodiesRight = new List<RigidBody>();

		public IslandManager()
			: base(new List<CollisionIsland>()) => islands = Items as List<CollisionIsland>;

		public void ArbiterCreated(Arbiter arbiter) {
			AddConnection(arbiter.body1, arbiter.body2);

			arbiter.body1.arbiters.Add(arbiter);
			arbiter.body2.arbiters.Add(arbiter);

			if(arbiter.body1.island != null)
				arbiter.body1.island.arbiter.Add(arbiter);
			else if(arbiter.body2.island != null)
				arbiter.body2.island.arbiter.Add(arbiter);
		}

		public void ArbiterRemoved(Arbiter arbiter) {
			arbiter.body1.arbiters.Remove(arbiter);
			arbiter.body2.arbiters.Remove(arbiter);

			if(arbiter.body1.island != null)
				arbiter.body1.island.arbiter.Remove(arbiter);
			else if(arbiter.body2.island != null)
				arbiter.body2.island.arbiter.Remove(arbiter);

			RemoveConnection(arbiter.body1, arbiter.body2);
		}

		public void ConstraintCreated(Constraint constraint) {
			AddConnection(constraint.body1, constraint.body2);

			constraint.body1.constraints.Add(constraint);
			if(constraint.body2 != null) constraint.body2.constraints.Add(constraint);

			if(constraint.body1.island != null)
				constraint.body1.island.constraints.Add(constraint);
			else if(constraint.body2 != null && constraint.body2.island != null)
				constraint.body2.island.constraints.Add(constraint);
		}

		public void ConstraintRemoved(Constraint constraint) {
			constraint.body1.constraints.Remove(constraint);

			if(constraint.body2 != null)
				constraint.body2.constraints.Remove(constraint);

			if(constraint.body1.island != null)
				constraint.body1.island.constraints.Remove(constraint);
			else if(constraint.body2 != null && constraint.body2.island != null)
				constraint.body2.island.constraints.Remove(constraint);

			RemoveConnection(constraint.body1, constraint.body2);
		}

		public void MakeBodyStatic(RigidBody body) {
			foreach(var b in body.connections) rmStackRb.Push(b);
			while(rmStackRb.Count > 0) RemoveConnection(body, rmStackRb.Pop());

			// A static body doesn't have any connections.
			body.connections.Clear();

			if(body.island != null) {
				body.island.bodies.Remove(body);

				if(body.island.bodies.Count == 0) {
					body.island.ClearLists();
					Pool.GiveBack(body.island);
				}
			}

			body.island = null;
		}

		public void RemoveBody(RigidBody body) {
			// Remove everything.
			foreach(var arbiter in body.arbiters) rmStackArb.Push(arbiter);
			while(rmStackArb.Count > 0) ArbiterRemoved(rmStackArb.Pop());

			foreach(var constraint in body.constraints) rmStackCstr.Push(constraint);
			while(rmStackCstr.Count > 0) ConstraintRemoved(rmStackCstr.Pop());

			body.arbiters.Clear();
			body.constraints.Clear();

			if(body.island != null) {
				Debug.Assert(body.island.islandManager == this,
					"IslandManager Inconsistency: IslandManager doesn't own the Island.");


				// the body should now form an island on his own.
				// thats okay, but since static bodies dont have islands
				// remove this island.
				Debug.Assert(body.island.bodies.Count == 1,
					"IslandManager Inconsistency: Removed all connections of a body - body is still in a non single Island.");

				body.island.ClearLists();
				Pool.GiveBack(body.island);

				islands.Remove(body.island);

				body.island = null;
			}
		}


		public void RemoveAll() {
			foreach(var island in islands) {
				foreach(var body in island.bodies) {
					body.arbiters.Clear();
					body.constraints.Clear();
					body.connections.Clear();
					body.island = null;
				}

				island.ClearLists();
			}

			islands.Clear();
		}

		void AddConnection(RigidBody body1, RigidBody body2) {
			Debug.Assert(!(body1.isStatic && body2.isStatic),
				"IslandManager Inconsistency: Arbiter detected between two static objects.");

			if(body1.isStatic) // <- only body1 is static
			{
				if(body2.island == null) {
					var newIsland = Pool.GetNew();
					newIsland.islandManager = this;

					body2.island = newIsland;
					body2.island.bodies.Add(body2);
					islands.Add(newIsland);
				}
			} else if(body2 == null || body2.isStatic) // <- only body2 is static
			{
				if(body1.island == null) {
					var newIsland = Pool.GetNew();
					newIsland.islandManager = this;

					body1.island = newIsland;
					body1.island.bodies.Add(body1);
					islands.Add(newIsland);
				}
			} else // both are !static
			{
				MergeIslands(body1, body2);

				body1.connections.Add(body2);
				body2.connections.Add(body1);
			}
		}

		void RemoveConnection(RigidBody body1, RigidBody body2) {
			Debug.Assert(!(body1.isStatic && body2.isStatic),
				"IslandManager Inconsistency: Arbiter detected between two static objects.");

			if(body1.isStatic) // <- only body1 is static
				body2.connections.Remove(body1);
			else if(body2 == null || body2.isStatic) // <- only body2 is static
				body1.connections.Remove(body2);
			else // <- both are !static
			{
				Debug.Assert(body1.island == body2.island,
					"IslandManager Inconsistency: Removing arbiter with different islands.");

				body1.connections.Remove(body2);
				body2.connections.Remove(body1);

				SplitIslands(body1, body2);
			}
		}

		void SplitIslands(RigidBody body0, RigidBody body1) {
			Debug.Assert(body0.island != null && body0.island == body1.island,
				"Islands not the same or null.");

			leftSearchQueue.Enqueue(body0);
			rightSearchQueue.Enqueue(body1);

			visitedBodiesLeft.Add(body0);
			visitedBodiesRight.Add(body1);

			body0.marker = 1;
			body1.marker = 2;

			while(leftSearchQueue.Count > 0 && rightSearchQueue.Count > 0) {
				var currentNode = leftSearchQueue.Dequeue();
				if(!currentNode.isStatic)
					for(var i = 0; i < currentNode.connections.Count; i++) {
						var connectedNode = currentNode.connections[i];

						if(connectedNode.marker == 0) {
							leftSearchQueue.Enqueue(connectedNode);
							visitedBodiesLeft.Add(connectedNode);
							connectedNode.marker = 1;
						} else if(connectedNode.marker == 2) {
							leftSearchQueue.Clear();
							rightSearchQueue.Clear();
							goto ResetSearchStates;
						}
					}

				currentNode = rightSearchQueue.Dequeue();
				if(!currentNode.isStatic)
					for(var i = 0; i < currentNode.connections.Count; i++) {
						var connectedNode = currentNode.connections[i];

						if(connectedNode.marker == 0) {
							rightSearchQueue.Enqueue(connectedNode);
							visitedBodiesRight.Add(connectedNode);
							connectedNode.marker = 2;
						} else if(connectedNode.marker == 1) {
							leftSearchQueue.Clear();
							rightSearchQueue.Clear();
							goto ResetSearchStates;
						}
					}
			}

			var island = Pool.GetNew();
			island.islandManager = this;

			islands.Add(island);

			if(leftSearchQueue.Count == 0) {
				for(var i = 0; i < visitedBodiesLeft.Count; i++) {
					var body = visitedBodiesLeft[i];
					body1.island.bodies.Remove(body);
					island.bodies.Add(body);
					body.island = island;

					foreach(var a in body.arbiters) {
						body1.island.arbiter.Remove(a);
						island.arbiter.Add(a);
					}

					foreach(var c in body.constraints) {
						body1.island.constraints.Remove(c);
						island.constraints.Add(c);
					}
				}

				rightSearchQueue.Clear();
			} else if(rightSearchQueue.Count == 0) {
				for(var i = 0; i < visitedBodiesRight.Count; i++) {
					var body = visitedBodiesRight[i];
					body0.island.bodies.Remove(body);
					island.bodies.Add(body);
					body.island = island;

					foreach(var a in body.arbiters) {
						body0.island.arbiter.Remove(a);
						island.arbiter.Add(a);
					}

					foreach(var c in body.constraints) {
						body0.island.constraints.Remove(c);
						island.constraints.Add(c);
					}
				}

				leftSearchQueue.Clear();
			}

			ResetSearchStates:

			for(var i = 0; i < visitedBodiesLeft.Count; i++) visitedBodiesLeft[i].marker = 0;

			for(var i = 0; i < visitedBodiesRight.Count; i++) visitedBodiesRight[i].marker = 0;

			visitedBodiesLeft.Clear();
			visitedBodiesRight.Clear();
		}

		// Boths bodies must be !static
		void MergeIslands(RigidBody body0, RigidBody body1) {
			if(body0.island != body1.island) // <- both bodies are in different islands
			{
				if(body0.island == null) // <- one island is null
				{
					body0.island = body1.island;
					body0.island.bodies.Add(body0);
				} else if(body1.island == null) // <- one island is null
				{
					body1.island = body0.island;
					body1.island.bodies.Add(body1);
				} else // <- both islands are different,
				{
					// merge smaller into larger
					RigidBody smallIslandOwner, largeIslandOwner;

					if(body0.island.bodies.Count > body1.island.bodies.Count) {
						smallIslandOwner = body1;
						largeIslandOwner = body0;
					} else {
						smallIslandOwner = body0;
						largeIslandOwner = body1;
					}

					var giveBackIsland = smallIslandOwner.island;

					Pool.GiveBack(giveBackIsland);
					islands.Remove(giveBackIsland);

					foreach(var b in giveBackIsland.bodies) {
						b.island = largeIslandOwner.island;
						largeIslandOwner.island.bodies.Add(b);
					}

					foreach(var a in giveBackIsland.arbiter) largeIslandOwner.island.arbiter.Add(a);

					foreach(var c in giveBackIsland.constraints) largeIslandOwner.island.constraints.Add(c);

					giveBackIsland.ClearLists();
				}
			} else if(body0.island == null) // <- both are null
			{
				var island = Pool.GetNew();
				island.islandManager = this;

				body0.island = body1.island = island;

				body0.island.bodies.Add(body0);
				body0.island.bodies.Add(body1);

				islands.Add(island);
			}
		}
	}
}