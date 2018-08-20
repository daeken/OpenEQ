/*
* Jitter Physics
* Copyright (c) 2011 Thorben Linneweber
* made 3d
* Added DynamicTree vs DynamicTree collision query
* 
* Farseer Physics Engine based on Box2D.XNA port:
* Copyright (c) 2010 Ian Qvist
* 
* Box2D.XNA port of Box2D:
* Copyright (c) 2009 Brandon Furtwangler, Nathan Furtwangler
*
* Original source Box2D:
* Copyright (c) 2006-2009 Erin Catto http://www.box2d.org 
* 
* This software is provided 'as-is', without any express or implied 
* warranty.  In no event will the authors be held liable for any damages 
* arising from the use of this software. 
* Permission is granted to anyone to use this software for any purpose, 
* including commercial applications, and to alter it and redistribute it 
* freely, subject to the following restrictions: 
* 1. The origin of this software must not be misrepresented; you must not 
* claim that you wrote the original software. If you use this software 
* in a product, an acknowledgment in the product documentation would be 
* appreciated but is not required. 
* 2. Altered source versions must be plainly marked as such, and must not be 
* misrepresented as being the original software. 
* 3. This notice may not be removed or altered from any source distribution. 
*/

using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Numerics;
using Jitter.LinearMath;

namespace Jitter.Collision {
    /// <summary>
    ///     A node in the dynamic tree. The client does not interact with this directly.
    /// </summary>
    public struct DynamicTreeNode<T> {
        /// <summary>
        ///     This is the fattened AABB.
        /// </summary>
        public JBBox AABB;

		public float MinorRandomExtension;

		public int Child1;
		public int Child2;

		public int LeafCount;
		public int ParentOrNext;
		public T UserData;

		public bool IsLeaf() => Child1 == DynamicTree<T>.NullNode;
	}

    /// <summary>
    ///     A dynamic tree arranges data in a binary tree to accelerate
    ///     queries such as volume queries and ray casts. Leafs are proxies
    ///     with an AABB. In the tree we expand the proxy AABB by Settings.b2_fatAABBFactor
    ///     so that the proxy AABB is bigger than the client object. This allows the client
    ///     object to move by small amounts without triggering a tree update.
    ///     Nodes are pooled and relocatable, so we use node indices rather than pointers.
    /// </summary>
    public class DynamicTree<T> {
		internal const int NullNode = -1;

		const float SettingsAABBMultiplier = 2.0f;
		int _freeList;
		int _insertionCount;
		int _nodeCapacity;
		int _nodeCount;

		readonly Random rnd = new Random();

		// Added by 'noone' to prevent highly symmetric cases to
		// update the whole tree at once.
		readonly float settingsRndExtension = 0.1f;


		readonly ResourcePool<Stack<int>> stackPool = new ResourcePool<Stack<int>>();

		public DynamicTree()
			: this(0.1f) {
		}

        /// <summary>
        ///     Constructing the tree initializes the node pool.
        /// </summary>
        public DynamicTree(float rndExtension) {
			settingsRndExtension = rndExtension;
			Root = NullNode;

			_nodeCapacity = 16;
			Nodes = new DynamicTreeNode<T>[_nodeCapacity];

			// Build a linked list for the free list.
			for(var i = 0; i < _nodeCapacity - 1; ++i) Nodes[i].ParentOrNext = i + 1;
			Nodes[_nodeCapacity - 1].ParentOrNext = NullNode;
		}

		public int Root { get; set; }

		public DynamicTreeNode<T>[] Nodes { get; set; }

        /// <summary>
        ///     Create a proxy in the tree as a leaf node. We return the index
        ///     of the node instead of a pointer so that we can grow
        ///     the node pool.
        ///     ///
        /// </summary>
        /// <param name="aabb">The aabb.</param>
        /// <param name="userData">The user data.</param>
        /// <returns>Index of the created proxy</returns>
        public int AddProxy(ref JBBox aabb, T userData) {
			var proxyId = AllocateNode();

			Nodes[proxyId].MinorRandomExtension = (float) rnd.NextDouble() * settingsRndExtension;

			// Fatten the aabb.
			var r = new Vector3(Nodes[proxyId].MinorRandomExtension);
			Nodes[proxyId].AABB.Min = aabb.Min - r;
			Nodes[proxyId].AABB.Max = aabb.Max + r;
			Nodes[proxyId].UserData = userData;
			Nodes[proxyId].LeafCount = 1;

			InsertLeaf(proxyId);

			return proxyId;
		}

        /// <summary>
        ///     Destroy a proxy. This asserts if the id is invalid.
        /// </summary>
        /// <param name="proxyId">The proxy id.</param>
        public void RemoveProxy(int proxyId) {
			Debug.Assert(0 <= proxyId && proxyId < _nodeCapacity);
			Debug.Assert(Nodes[proxyId].IsLeaf());

			RemoveLeaf(proxyId);
			FreeNode(proxyId);
		}

        /// <summary>
        ///     Move a proxy with a swepted AABB. If the proxy has moved outside of its fattened AABB,
        ///     then the proxy is removed from the tree and re-inserted. Otherwise
        ///     the function returns immediately.
        /// </summary>
        /// <param name="proxyId">The proxy id.</param>
        /// <param name="aabb">The aabb.</param>
        /// <param name="displacement">The displacement.</param>
        /// <returns>true if the proxy was re-inserted.</returns>
        public bool MoveProxy(int proxyId, ref JBBox aabb, Vector3 displacement) {
			Debug.Assert(0 <= proxyId && proxyId < _nodeCapacity);

			Debug.Assert(Nodes[proxyId].IsLeaf());

			if(Nodes[proxyId].AABB.Contains(ref aabb) != JBBox.ContainmentType.Disjoint) return false;

			RemoveLeaf(proxyId);

			// Extend AABB.
			var b = aabb;
			var r = new Vector3(Nodes[proxyId].MinorRandomExtension);
			b.Min = b.Min - r;
			b.Max = b.Max + r;

			// Predict AABB displacement.
			var d = SettingsAABBMultiplier * displacement;
			//Vector3 randomExpansion = new Vector3((float)rnd.Next(0, 10) * 0.1f, (float)rnd.Next(0, 10) * 0.1f, (float)rnd.Next(0, 10) * 0.1f);

			//d += randomExpansion;

			if(d.X < 0.0f)
				b.Min.X += d.X;
			else
				b.Max.X += d.X;

			if(d.Y < 0.0f)
				b.Min.Y += d.Y;
			else
				b.Max.Y += d.Y;

			if(d.Z < 0.0f)
				b.Min.Z += d.Z;
			else
				b.Max.Z += d.Z;

			Nodes[proxyId].AABB = b;

			InsertLeaf(proxyId);
			return true;
		}

        /// <summary>
        ///     Get proxy user data.
        /// </summary>
        /// <typeparam name="T"></typeparam>
        /// <param name="proxyId">The proxy id.</param>
        /// <returns>the proxy user data or 0 if the id is invalid.</returns>
        public T GetUserData(int proxyId) {
			Debug.Assert(0 <= proxyId && proxyId < _nodeCapacity);
			return Nodes[proxyId].UserData;
		}

        /// <summary>
        ///     Get the fat AABB for a proxy.
        /// </summary>
        /// <param name="proxyId">The proxy id.</param>
        /// <param name="fatAABB">The fat AABB.</param>
        public void GetFatAABB(int proxyId, out JBBox fatAABB) {
			Debug.Assert(0 <= proxyId && proxyId < _nodeCapacity);
			fatAABB = Nodes[proxyId].AABB;
		}

        /// <summary>
        ///     Compute the height of the binary tree in O(N) time. Should not be
        ///     called often.
        /// </summary>
        /// <returns></returns>
        public int ComputeHeight() => ComputeHeight(Root);

		public void Query(Vector3 origin, Vector3 direction, List<int> collisions) {
			var stack = stackPool.GetNew();

			stack.Push(Root);

			while(stack.Count > 0) {
				var nodeId = stack.Pop();
				var node = Nodes[nodeId];

				if(node.AABB.RayIntersect(ref origin, ref direction)) {
					if(node.IsLeaf()) collisions.Add(nodeId);
					else {
						if(Nodes[node.Child1].AABB.RayIntersect(ref origin, ref direction)) stack.Push(node.Child1);
						if(Nodes[node.Child2].AABB.RayIntersect(ref origin, ref direction)) stack.Push(node.Child2);
					}
				}
			}

			stackPool.GiveBack(stack);
		}

		public void Query(List<int> other, List<int> my, DynamicTree<T> tree) {
			var stack1 = stackPool.GetNew();
			var stack2 = stackPool.GetNew();

			stack1.Push(Root);
			stack2.Push(tree.Root);

			while(stack1.Count > 0) {
				var nodeId1 = stack1.Pop();
				var nodeId2 = stack2.Pop();

				if(nodeId1 == NullNode) continue;
				if(nodeId2 == NullNode) continue;

				if(tree.Nodes[nodeId2].AABB.Contains(ref Nodes[nodeId1].AABB) != JBBox.ContainmentType.Disjoint) {
					if(Nodes[nodeId1].IsLeaf() && tree.Nodes[nodeId2].IsLeaf()) {
						my.Add(nodeId1);
						other.Add(nodeId2);
					} else if(tree.Nodes[nodeId2].IsLeaf()) {
						stack1.Push(Nodes[nodeId1].Child1);
						stack2.Push(nodeId2);

						stack1.Push(Nodes[nodeId1].Child2);
						stack2.Push(nodeId2);
					} else if(Nodes[nodeId1].IsLeaf()) {
						stack1.Push(nodeId1);
						stack2.Push(tree.Nodes[nodeId2].Child1);

						stack1.Push(nodeId1);
						stack2.Push(tree.Nodes[nodeId2].Child2);
					} else {
						stack1.Push(Nodes[nodeId1].Child1);
						stack2.Push(tree.Nodes[nodeId2].Child1);

						stack1.Push(Nodes[nodeId1].Child1);
						stack2.Push(tree.Nodes[nodeId2].Child2);

						stack1.Push(Nodes[nodeId1].Child2);
						stack2.Push(tree.Nodes[nodeId2].Child1);

						stack1.Push(Nodes[nodeId1].Child2);
						stack2.Push(tree.Nodes[nodeId2].Child2);
					}
				}
			}

			stackPool.GiveBack(stack1);
			stackPool.GiveBack(stack2);
		}

        /// <summary>
        ///     Query an AABB for overlapping proxies. The callback class
        ///     is called for each proxy that overlaps the supplied AABB.
        /// </summary>
        /// <param name="callback">The callback.</param>
        /// <param name="aabb">The aabb.</param>
        public void Query(List<int> my, ref JBBox aabb) {
			//Stack<int> _stack = new Stack<int>(256);
			var _stack = stackPool.GetNew();

			_stack.Push(Root);

			while(_stack.Count > 0) {
				var nodeId = _stack.Pop();
				if(nodeId == NullNode) continue;

				var node = Nodes[nodeId];

				//if (JBBox.TestOverlap(ref node.AABB, ref aabb))
				if(aabb.Contains(ref node.AABB) != JBBox.ContainmentType.Disjoint) {
					if(node.IsLeaf())
						my.Add(nodeId);
					else {
						_stack.Push(node.Child1);
						_stack.Push(node.Child2);
					}
				}
			}

			stackPool.GiveBack(_stack);
		}

		int CountLeaves(int nodeId) {
			if(nodeId == NullNode) return 0;

			Debug.Assert(0 <= nodeId && nodeId < _nodeCapacity);
			var node = Nodes[nodeId];

			if(node.IsLeaf()) {
				Debug.Assert(node.LeafCount == 1);
				return 1;
			}

			var count1 = CountLeaves(node.Child1);
			var count2 = CountLeaves(node.Child2);
			var count = count1 + count2;
			Debug.Assert(count == node.LeafCount);
			return count;
		}

		void Validate() {
			CountLeaves(Root);
		}

		int AllocateNode() {
			// Expand the node pool as needed.
			if(_freeList == NullNode) {
				Debug.Assert(_nodeCount == _nodeCapacity);

				// The free list is empty. Rebuild a bigger pool.
				var oldNodes = Nodes;
				_nodeCapacity *= 2;
				Nodes = new DynamicTreeNode<T>[_nodeCapacity];
				Array.Copy(oldNodes, Nodes, _nodeCount);

				// Build a linked list for the free list. The parent
				// pointer becomes the "next" pointer.
				for(var i = _nodeCount; i < _nodeCapacity - 1; ++i) Nodes[i].ParentOrNext = i + 1;
				Nodes[_nodeCapacity - 1].ParentOrNext = NullNode;
				_freeList = _nodeCount;
			}

			// Peel a node off the free list.
			var nodeId = _freeList;
			_freeList = Nodes[nodeId].ParentOrNext;
			Nodes[nodeId].ParentOrNext = NullNode;
			Nodes[nodeId].Child1 = NullNode;
			Nodes[nodeId].Child2 = NullNode;
			Nodes[nodeId].LeafCount = 0;
			++_nodeCount;
			return nodeId;
		}

		void FreeNode(int nodeId) {
			Debug.Assert(0 <= nodeId && nodeId < _nodeCapacity);
			Debug.Assert(0 < _nodeCount);
			Nodes[nodeId].ParentOrNext = _freeList;
			_freeList = nodeId;
			--_nodeCount;
		}

		void InsertLeaf(int leaf) {
			++_insertionCount;

			if(Root == NullNode) {
				Root = leaf;
				Nodes[Root].ParentOrNext = NullNode;
				return;
			}

			// Find the best sibling for this node
			var leafAABB = Nodes[leaf].AABB;
			var sibling = Root;
			while(Nodes[sibling].IsLeaf() == false) {
				var child1 = Nodes[sibling].Child1;
				var child2 = Nodes[sibling].Child2;

				// Expand the node's AABB.
				//_nodes[sibling].AABB.Combine(ref leafAABB);
				JBBox.CreateMerged(ref Nodes[sibling].AABB, ref leafAABB, out Nodes[sibling].AABB);

				Nodes[sibling].LeafCount += 1;

				var siblingArea = Nodes[sibling].AABB.Perimeter;
				var parentAABB = new JBBox();
				//parentAABB.Combine(ref _nodes[sibling].AABB, ref leafAABB);
				JBBox.CreateMerged(ref Nodes[sibling].AABB, ref leafAABB, out Nodes[sibling].AABB);

				var parentArea = parentAABB.Perimeter;
				var cost1 = 2.0f * parentArea;

				var inheritanceCost = 2.0f * (parentArea - siblingArea);

				float cost2;
				if(Nodes[child1].IsLeaf()) {
					var aabb = new JBBox();
					//aabb.Combine(ref leafAABB, ref _nodes[child1].AABB);
					JBBox.CreateMerged(ref leafAABB, ref Nodes[child1].AABB, out aabb);
					cost2 = aabb.Perimeter + inheritanceCost;
				} else {
					var aabb = new JBBox();
					//aabb.Combine(ref leafAABB, ref _nodes[child1].AABB);
					JBBox.CreateMerged(ref leafAABB, ref Nodes[child1].AABB, out aabb);

					var oldArea = Nodes[child1].AABB.Perimeter;
					var newArea = aabb.Perimeter;
					cost2 = newArea - oldArea + inheritanceCost;
				}

				float cost3;
				if(Nodes[child2].IsLeaf()) {
					var aabb = new JBBox();
					//aabb.Combine(ref leafAABB, ref _nodes[child2].AABB);
					JBBox.CreateMerged(ref leafAABB, ref Nodes[child2].AABB, out aabb);
					cost3 = aabb.Perimeter + inheritanceCost;
				} else {
					var aabb = new JBBox();
					//aabb.Combine(ref leafAABB, ref _nodes[child2].AABB);
					JBBox.CreateMerged(ref leafAABB, ref Nodes[child2].AABB, out aabb);
					var oldArea = Nodes[child2].AABB.Perimeter;
					var newArea = aabb.Perimeter;
					cost3 = newArea - oldArea + inheritanceCost;
				}

				// Descend according to the minimum cost.
				if(cost1 < cost2 && cost1 < cost3) break;

				// Expand the node's AABB to account for the new leaf.
				//_nodes[sibling].AABB.Combine(ref leafAABB);
				JBBox.CreateMerged(ref leafAABB, ref Nodes[sibling].AABB, out Nodes[sibling].AABB);

				// Descend
				if(cost2 < cost3)
					sibling = child1;
				else
					sibling = child2;
			}

			// Create a new parent for the siblings.
			var oldParent = Nodes[sibling].ParentOrNext;
			var newParent = AllocateNode();
			Nodes[newParent].ParentOrNext = oldParent;
			Nodes[newParent].UserData = default;
			//_nodes[newParent].AABB.Combine(ref leafAABB, ref _nodes[sibling].AABB);
			JBBox.CreateMerged(ref leafAABB, ref Nodes[sibling].AABB, out Nodes[newParent].AABB);
			Nodes[newParent].LeafCount = Nodes[sibling].LeafCount + 1;

			if(oldParent != NullNode) {
				// The sibling was not the root.
				if(Nodes[oldParent].Child1 == sibling)
					Nodes[oldParent].Child1 = newParent;
				else
					Nodes[oldParent].Child2 = newParent;

				Nodes[newParent].Child1 = sibling;
				Nodes[newParent].Child2 = leaf;
				Nodes[sibling].ParentOrNext = newParent;
				Nodes[leaf].ParentOrNext = newParent;
			} else {
				// The sibling was the root.
				Nodes[newParent].Child1 = sibling;
				Nodes[newParent].Child2 = leaf;
				Nodes[sibling].ParentOrNext = newParent;
				Nodes[leaf].ParentOrNext = newParent;
				Root = newParent;
			}
		}

		void RemoveLeaf(int leaf) {
			if(leaf == Root) {
				Root = NullNode;
				return;
			}

			var parent = Nodes[leaf].ParentOrNext;
			var grandParent = Nodes[parent].ParentOrNext;
			int sibling;
			if(Nodes[parent].Child1 == leaf)
				sibling = Nodes[parent].Child2;
			else
				sibling = Nodes[parent].Child1;

			if(grandParent != NullNode) {
				// Destroy parent and connect sibling to grandParent.
				if(Nodes[grandParent].Child1 == parent)
					Nodes[grandParent].Child1 = sibling;
				else
					Nodes[grandParent].Child2 = sibling;
				Nodes[sibling].ParentOrNext = grandParent;
				FreeNode(parent);

				// Adjust ancestor bounds.
				parent = grandParent;
				while(parent != NullNode) {
					//_nodes[parent].AABB.Combine(ref _nodes[_nodes[parent].Child1].AABB,
					//                            ref _nodes[_nodes[parent].Child2].AABB);

					JBBox.CreateMerged(ref Nodes[Nodes[parent].Child1].AABB,
						ref Nodes[Nodes[parent].Child2].AABB, out Nodes[parent].AABB);

					Debug.Assert(Nodes[parent].LeafCount > 0);
					Nodes[parent].LeafCount -= 1;

					parent = Nodes[parent].ParentOrNext;
				}
			} else {
				Root = sibling;
				Nodes[sibling].ParentOrNext = NullNode;
				FreeNode(parent);
			}
		}

		int ComputeHeight(int nodeId) {
			if(nodeId == NullNode) return 0;

			Debug.Assert(0 <= nodeId && nodeId < _nodeCapacity);
			var node = Nodes[nodeId];
			var height1 = ComputeHeight(node.Child1);
			var height2 = ComputeHeight(node.Child2);
			return 1 + Math.Max(height1, height2);
		}
	}
}