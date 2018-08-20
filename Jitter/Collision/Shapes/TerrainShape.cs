/* Copyright (C) <2009-2011> <Thorben Linneweber, Jitter Physics>
* 
*  This software is provided 'as-is', without any express or implied
*  warranty.  In no event will the authors be held liable for any damages
*  arising from the use of this software.
*
*  Permission is granted to anyone to use this software for any purpose,
*  including commercial applications, and to alter it and redistribute it
*  freely, subject to the following restrictions:
*
*  1. The origin of this software must not be misrepresented; you must not
*      claim that you wrote the original software. If you use this software
*      in a product, an acknowledgment in the product documentation would be
*      appreciated but is not required.
*  2. Altered source versions must be plainly marked as such, and must not be
*      misrepresented as being the original software.
*  3. This notice may not be removed or altered from any source distribution. 
*/

#region Using Statements

using System;
using System.Collections.Generic;
using System.Numerics;
using Jitter.LinearMath;

#endregion

namespace Jitter.Collision.Shapes {
    /// <summary>
    ///     Represents a terrain.
    /// </summary>
    public class TerrainShape : Multishape {
		JBBox boundings;
		float[,] heights;
		int heightsLength0, heightsLength1;

		int minX, maxX;
		int minZ, maxZ;
		Vector3 normal = new Vector3(0, 1, 0);
		int numX, numZ;


		readonly Vector3[] points = new Vector3[3];
		float scaleX, scaleZ;

        /// <summary>
        ///     Initializes a new instance of the TerrainShape class.
        /// </summary>
        /// <param name="heights">An array containing the heights of the terrain surface.</param>
        /// <param name="scaleX">The x-scale factor. (The x-space between neighbour heights)</param>
        /// <param name="scaleZ">The y-scale factor. (The y-space between neighbour heights)</param>
        public TerrainShape(float[,] heights, float scaleX, float scaleZ) {
			heightsLength0 = heights.GetLength(0);
			heightsLength1 = heights.GetLength(1);

			#region Bounding Box

			boundings = JBBox.SmallBox;

			for(var i = 0; i < heightsLength0; i++) {
				for(var e = 0; e < heightsLength1; e++)
					if(heights[i, e] > boundings.Max.Y)
						boundings.Max.Y = heights[i, e];
					else if(heights[i, e] < boundings.Min.Y)
						boundings.Min.Y = heights[i, e];
			}

			boundings.Min.X = 0.0f;
			boundings.Min.Z = 0.0f;

			boundings.Max.X = heightsLength0 * scaleX;
			boundings.Max.Z = heightsLength1 * scaleZ;

			#endregion

			this.heights = heights;
			this.scaleX = scaleX;
			this.scaleZ = scaleZ;

			UpdateShape();
		}

		internal TerrainShape() {
		}

        /// <summary>
        ///     Expands the triangles by the specified amount.
        ///     This stabilizes collision detection for flat shapes.
        /// </summary>
        public float SphericalExpansion { get; set; } = 0.05f;


		protected override Multishape CreateWorkingClone() {
			var clone = new TerrainShape();
			clone.heights = heights;
			clone.scaleX = scaleX;
			clone.scaleZ = scaleZ;
			clone.boundings = boundings;
			clone.heightsLength0 = heightsLength0;
			clone.heightsLength1 = heightsLength1;
			clone.SphericalExpansion = SphericalExpansion;
			return clone;
		}

        /// <summary>
        ///     Sets the current shape. First <see cref="Prepare" /> has to be called.
        ///     After SetCurrentShape the shape immitates another shape.
        /// </summary>
        /// <param name="index"></param>
        public override void SetCurrentShape(int index) {
			var leftTriangle = false;

			if(index >= numX * numZ) {
				leftTriangle = true;
				index -= numX * numZ;
			}

			var quadIndexX = index % numX;
			var quadIndexZ = index / numX;

			// each quad has two triangles, called 'leftTriangle' and !'leftTriangle'
			if(leftTriangle) {
				points[0].Set((minX + quadIndexX + 0) * scaleX, heights[minX + quadIndexX + 0, minZ + quadIndexZ + 0],
					(minZ + quadIndexZ + 0) * scaleZ);
				points[1].Set((minX + quadIndexX + 1) * scaleX, heights[minX + quadIndexX + 1, minZ + quadIndexZ + 0],
					(minZ + quadIndexZ + 0) * scaleZ);
				points[2].Set((minX + quadIndexX + 0) * scaleX, heights[minX + quadIndexX + 0, minZ + quadIndexZ + 1],
					(minZ + quadIndexZ + 1) * scaleZ);
			} else {
				points[0].Set((minX + quadIndexX + 1) * scaleX, heights[minX + quadIndexX + 1, minZ + quadIndexZ + 0],
					(minZ + quadIndexZ + 0) * scaleZ);
				points[1].Set((minX + quadIndexX + 1) * scaleX, heights[minX + quadIndexX + 1, minZ + quadIndexZ + 1],
					(minZ + quadIndexZ + 1) * scaleZ);
				points[2].Set((minX + quadIndexX + 0) * scaleX, heights[minX + quadIndexX + 0, minZ + quadIndexZ + 1],
					(minZ + quadIndexZ + 1) * scaleZ);
			}

			geomCen = (points[0] + points[1] + points[2]) / 3;

			normal = Vector3.Cross(points[1] - points[0], points[2] - points[0]);
		}

		public void CollisionNormal(out Vector3 normal) {
			normal = this.normal;
		}


        /// <summary>
        ///     Passes a axis aligned bounding box to the shape where collision
        ///     could occour.
        /// </summary>
        /// <param name="box">The bounding box where collision could occur.</param>
        /// <returns>
        ///     The upper index with which <see cref="SetCurrentShape" /> can be
        ///     called.
        /// </returns>
        public override int Prepare(ref JBBox box) {
			// simple idea: the terrain is a grid. x and z is the position in the grid.
			// y the height. we know compute the min and max grid-points. All quads
			// between these points have to be checked.

			// including overflow exception prevention

			if(box.Min.X < boundings.Min.X) minX = 0;
			else {
				minX = (int) MathF.Floor((box.Min.X - SphericalExpansion) / scaleX);
				minX = Math.Max(minX, 0);
			}

			if(box.Max.X > boundings.Max.X) maxX = heightsLength0 - 1;
			else {
				maxX = (int) MathF.Ceiling((box.Max.X + SphericalExpansion) / scaleX);
				maxX = Math.Min(maxX, heightsLength0 - 1);
			}

			if(box.Min.Z < boundings.Min.Z) minZ = 0;
			else {
				minZ = (int) MathF.Floor((box.Min.Z - SphericalExpansion) / scaleZ);
				minZ = Math.Max(minZ, 0);
			}

			if(box.Max.Z > boundings.Max.Z) maxZ = heightsLength1 - 1;
			else {
				maxZ = (int) MathF.Ceiling((box.Max.Z + SphericalExpansion) / scaleZ);
				maxZ = Math.Min(maxZ, heightsLength1 - 1);
			}

			numX = maxX - minX;
			numZ = maxZ - minZ;

			// since every quad contains two triangles we multiply by 2.
			return numX * numZ * 2;
		}

        /// <summary>
        /// </summary>
        public override void CalculateMassInertia() {
			inertia = JMatrix.Identity;
			Mass = 1.0f;
		}

        /// <summary>
        ///     Gets the axis aligned bounding box of the orientated shape. This includes
        ///     the whole shape.
        /// </summary>
        /// <param name="orientation">The orientation of the shape.</param>
        /// <param name="box">The axis aligned bounding box of the shape.</param>
        public override void GetBoundingBox(ref JMatrix orientation, out JBBox box) {
			box = boundings;

			#region Expand Spherical

			box.Min.X -= SphericalExpansion;
			box.Min.Y -= SphericalExpansion;
			box.Min.Z -= SphericalExpansion;
			box.Max.X += SphericalExpansion;
			box.Max.Y += SphericalExpansion;
			box.Max.Z += SphericalExpansion;

			#endregion

			box.Transform(ref orientation);
		}

		public override void MakeHull(ref List<Vector3> triangleList, int generationThreshold) {
			for(var index = 0; index < (heightsLength0 - 1) * (heightsLength1 - 1); index++) {
				var quadIndexX = index % (heightsLength0 - 1);
				var quadIndexZ = index / (heightsLength0 - 1);

				triangleList.Add(new Vector3((0 + quadIndexX + 0) * scaleX,
					heights[0 + quadIndexX + 0, 0 + quadIndexZ + 0], (0 + quadIndexZ + 0) * scaleZ));
				triangleList.Add(new Vector3((0 + quadIndexX + 1) * scaleX,
					heights[0 + quadIndexX + 1, 0 + quadIndexZ + 0], (0 + quadIndexZ + 0) * scaleZ));
				triangleList.Add(new Vector3((0 + quadIndexX + 0) * scaleX,
					heights[0 + quadIndexX + 0, 0 + quadIndexZ + 1], (0 + quadIndexZ + 1) * scaleZ));

				triangleList.Add(new Vector3((0 + quadIndexX + 1) * scaleX,
					heights[0 + quadIndexX + 1, 0 + quadIndexZ + 0], (0 + quadIndexZ + 0) * scaleZ));
				triangleList.Add(new Vector3((0 + quadIndexX + 1) * scaleX,
					heights[0 + quadIndexX + 1, 0 + quadIndexZ + 1], (0 + quadIndexZ + 1) * scaleZ));
				triangleList.Add(new Vector3((0 + quadIndexX + 0) * scaleX,
					heights[0 + quadIndexX + 0, 0 + quadIndexZ + 1], (0 + quadIndexZ + 1) * scaleZ));
			}
		}

        /// <summary>
        ///     SupportMapping. Finds the point in the shape furthest away from the given direction.
        ///     Imagine a plane with a normal in the search direction. Now move the plane along the normal
        ///     until the plane does not intersect the shape. The last intersection point is the result.
        /// </summary>
        /// <param name="direction">The direction.</param>
        /// <param name="result">The result.</param>
        public override void SupportMapping(ref Vector3 direction, out Vector3 result) {
			var expandVector = Vector3.Normalize(direction) * SphericalExpansion;

			var minIndex = 0;
			var min = Vector3.Dot(points[0], direction);
			var dot = Vector3.Dot(points[1], direction);
			if(dot > min) {
				min = dot;
				minIndex = 1;
			}

			dot = Vector3.Dot(points[2], direction);
			if(dot > min) {
				min = dot;
				minIndex = 2;
			}

			result = points[minIndex] + expandVector;
		}

        /// <summary>
        /// </summary>
        /// <param name="rayOrigin"></param>
        /// <param name="rayDelta"></param>
        /// <returns></returns>
        public override int Prepare(ref Vector3 rayOrigin, ref Vector3 rayDelta) {
			var box = JBBox.SmallBox;

			#region RayEnd + Expand Spherical

			var rayEnd = rayOrigin + rayDelta + Vector3.Normalize(rayDelta) * SphericalExpansion;

			#endregion

			box.AddPoint(ref rayOrigin);
			box.AddPoint(ref rayEnd);

			return Prepare(ref box);
		}
	}
}