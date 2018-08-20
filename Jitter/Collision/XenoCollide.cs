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

using System.Numerics;
using Jitter.LinearMath;

#endregion

namespace Jitter.Collision {
    /// <summary>
    ///     The implementation of the ISupportMappable interface defines the form
    ///     of a shape. <seealso cref="GJKCollide" /> <seealso cref="XenoCollide" />
    /// </summary>
    public interface ISupportMappable {
        /// <summary>
        ///     SupportMapping. Finds the point in the shape furthest away from the given direction.
        ///     Imagine a plane with a normal in the search direction. Now move the plane along the normal
        ///     until the plane does not intersect the shape. The last intersection point is the result.
        /// </summary>
        /// <param name="direction">The direction.</param>
        /// <param name="result">The result.</param>
        void SupportMapping(ref Vector3 direction, out Vector3 result);

        /// <summary>
        ///     The center of the SupportMap.
        /// </summary>
        /// <param name="center"></param>
        void SupportCenter(out Vector3 center);
	}

    /// <summary>
    ///     Implementation of the XenoCollide Algorithm by Gary Snethen.
    ///     Narrowphase collision detection highly optimized for C#.
    ///     http://xenocollide.snethen.com/
    /// </summary>
    public sealed class XenoCollide {
		const float CollideEpsilon = 1e-4f;
		const int MaximumIterations = 34;

		static void SupportMapTransformed(ISupportMappable support,
			ref JMatrix orientation, Vector3 position, Vector3 direction, out Vector3 result) {
			// THIS IS *THE* HIGH FREQUENCY CODE OF THE COLLLISION PART OF THE ENGINE

			result.X = direction.X * orientation.M11 + direction.Y * orientation.M12 + direction.Z * orientation.M13;
			result.Y = direction.X * orientation.M21 + direction.Y * orientation.M22 + direction.Z * orientation.M23;
			result.Z = direction.X * orientation.M31 + direction.Y * orientation.M32 + direction.Z * orientation.M33;

			support.SupportMapping(ref result, out result);

			var x = result.X * orientation.M11 + result.Y * orientation.M21 + result.Z * orientation.M31;
			var y = result.X * orientation.M12 + result.Y * orientation.M22 + result.Z * orientation.M32;
			var z = result.X * orientation.M13 + result.Y * orientation.M23 + result.Z * orientation.M33;

			result.X = position.X + x;
			result.Y = position.Y + y;
			result.Z = position.Z + z;
		}

        /// <summary>
        ///     Checks two shapes for collisions.
        /// </summary>
        /// <param name="support1">The SupportMappable implementation of the first shape to test.</param>
        /// <param name="support2">The SupportMappable implementation of the seconds shape to test.</param>
        /// <param name="orientation1">The orientation of the first shape.</param>
        /// <param name="orientation2">The orientation of the second shape.</param>
        /// <param name="position1">The position of the first shape.</param>
        /// <param name="position2">The position of the second shape</param>
        /// <param name="point">The pointin world coordinates, where collision occur.</param>
        /// <param name="normal">The normal pointing from body2 to body1.</param>
        /// <param name="penetration">Estimated penetration depth of the collision.</param>
        /// <returns>Returns true if there is a collision, false otherwise.</returns>
        public static bool Detect(ISupportMappable support1, ISupportMappable support2, ref JMatrix orientation1,
			ref JMatrix orientation2, Vector3 position1, Vector3 position2,
			out Vector3 point, out Vector3 normal, out float penetration) {
			// Used variables
			Vector3 temp1;
			Vector3 v01, v02, v0;
			Vector3 v11, v12, v1;
			Vector3 v21, v22, v2;
			Vector3 v31, v32, v3;
			Vector3 v41, v42, v4;
			Vector3 mn;

			// Initialization of the output
			point = normal = Vector3.Zero;
			penetration = 0.0f;

			//Vector3 right = Vector3.Right;

			// Get the center of shape1 in world coordinates -> v01
			support1.SupportCenter(out v01);
			v01 = position1 + v01.Transform(orientation1);

			// Get the center of shape2 in world coordinates -> v02
			support2.SupportCenter(out v02);
			v02 = position2 + v02.Transform(orientation2);

			// v0 is the center of the minkowski difference
			v0 = v02 - v01;

			// Avoid case where centers overlap -- any direction is fine in this case
			if(v0.IsNearlyZero()) v0 = new Vector3(0.00001f, 0, 0);

			// v1 = support in direction of origin
			mn = v0;
			normal = -v0;

			SupportMapTransformed(support1, ref orientation1, position1, mn, out v11);
			SupportMapTransformed(support2, ref orientation2, position2, normal, out v12);
			v1 = v12 - v11;

			if(Vector3.Dot(v1, normal) <= 0.0f) return false;

			// v2 = support perpendicular to v1,v0
			normal = Vector3.Cross(v1, v0);

			if(normal.IsNearlyZero()) {
				normal = v1 - v0;
				normal.Normalize();

				point = v11;
				point += v12;
				point /= 2;

				temp1 = v12 - v11;
				penetration = Vector3.Dot(temp1, normal);

				//point = v11;
				//point2 = v12;
				return true;
			}

			mn = -normal;
			SupportMapTransformed(support1, ref orientation1, position1, mn, out v21);
			SupportMapTransformed(support2, ref orientation2, position2, normal, out v22);
			v2 = v22 - v21;

			if(Vector3.Dot(v2, normal) <= 0.0f) return false;

			// Determine whether origin is on + or - side of plane (v1,v0,v2)
			normal = Vector3.Cross(v1 - v0, v2 - v0);

			var dist = Vector3.Dot(normal, v0);

			// If the origin is on the - side of the plane, reverse the direction of the plane
			if(dist > 0.0f) {
				Extensions.Swap(ref v1, ref v2);
				Extensions.Swap(ref v11, ref v21);
				Extensions.Swap(ref v12, ref v22);
				normal = -normal;
			}


			var phase2 = 0;
			var phase1 = 0;
			var hit = false;

			// Phase One: Identify a portal
			while(true) {
				if(phase1 > MaximumIterations) return false;

				phase1++;

				// Obtain the support point in a direction perpendicular to the existing plane
				// Note: This point is guaranteed to lie off the plane
				mn = -normal;
				SupportMapTransformed(support1, ref orientation1, position1, mn, out v31);
				SupportMapTransformed(support2, ref orientation2, position2, normal, out v32);
				v3 = v32 - v31;

				if(Vector3.Dot(v3, normal) <= 0.0f) return false;

				// If origin is outside (v1,v0,v3), then eliminate v2 and loop
				temp1 = Vector3.Cross(v1, v3);
				if(Vector3.Dot(temp1, v0) < 0.0f) {
					v2 = v3;
					v21 = v31;
					v22 = v32;
					normal = Vector3.Cross(v1 - v0, v3 - v0);
					continue;
				}

				// If origin is outside (v3,v0,v2), then eliminate v1 and loop
				temp1 = Vector3.Cross(v3, v2);
				if(Vector3.Dot(temp1, v0) < 0.0f) {
					v1 = v3;
					v11 = v31;
					v12 = v32;
					normal = Vector3.Cross(v3 - v0, v2 - v0);
					continue;
				}

				// Phase Two: Refine the portal
				// We are now inside of a wedge...
				while(true) {
					phase2++;

					// Compute normal of the wedge face
					normal = Vector3.Cross(v2 - v1, v3 - v1);

					// Can this happen???  Can it be handled more cleanly?
					if(normal.IsNearlyZero()) return true;

					normal.Normalize();

					// Compute distance from origin to wedge face
					var d = Vector3.Dot(normal, v1);


					// If the origin is inside the wedge, we have a hit
					if(d >= 0 && !hit) hit = true;

					// Find the support point in the direction of the wedge face
					mn = -normal;
					SupportMapTransformed(support1, ref orientation1, position1, mn, out v41);
					SupportMapTransformed(support2, ref orientation2, position2, normal, out v42);
					v4 = v42 - v41;

					temp1 = v4 - v3;
					var delta = Vector3.Dot(temp1, normal);
					penetration = Vector3.Dot(v4, normal);

					// If the boundary is thin enough or the origin is outside the support plane for the newly discovered vertex, then we can terminate
					if(delta <= CollideEpsilon || penetration <= 0.0f || phase2 > MaximumIterations) {
						if(hit) {
							var b0 = Vector3.Dot(Vector3.Cross(v1, v2), v3);
							var b1 = Vector3.Dot(Vector3.Cross(v3, v2), v0);
							var b2 = Vector3.Dot(Vector3.Cross(v0, v1), v3);
							var b3 = Vector3.Dot(Vector3.Cross(v2, v1), v0);

							var sum = b0 + b1 + b2 + b3;

							if(sum <= 0) {
								b0 = 0;
								b1 = Vector3.Dot(Vector3.Cross(v2, v3), normal);
								b2 = Vector3.Dot(Vector3.Cross(v3, v1), normal);
								b3 = Vector3.Dot(Vector3.Cross(v1, v2), normal);

								sum = b1 + b2 + b3;
							}

							var inv = 1.0f / sum;

							point = v01 * v0;
							point += v11 * v1;
							point += v21 * b2;
							point += v31 * v3;

							point += v02 * b0;
							point += v12 * b1;
							point += v22 * b2;
							point += v32 * b3;

							point *= inv * 0.5f;
						}

						// Compute the barycentric coordinates of the origin
						return hit;
					}

					//// Compute the tetrahedron dividing face (v4,v0,v1)
					//temp1 = Vector3.Cross(v4, v1);
					//float d1 = Vector3.Dot(temp1, v0);


					//// Compute the tetrahedron dividing face (v4,v0,v2)
					//temp1 = Vector3.Cross(v4, v2);
					//float d2 = Vector3.Dot(temp1, v0);


					// Compute the tetrahedron dividing face (v4,v0,v3)
					temp1 = Vector3.Cross(v4, v0);
					var dot = Vector3.Dot(temp1, v1);

					if(dot >= 0.0f) {
						dot = Vector3.Dot(temp1, v2);

						if(dot >= 0.0f) {
							// Inside d1 & inside d2 ==> eliminate v1
							v1 = v4;
							v11 = v41;
							v12 = v42;
						} else {
							// Inside d1 & outside d2 ==> eliminate v3
							v3 = v4;
							v31 = v41;
							v32 = v42;
						}
					} else {
						dot = Vector3.Dot(temp1, v3);

						if(dot >= 0.0f) {
							// Outside d1 & inside d3 ==> eliminate v2
							v2 = v4;
							v21 = v41;
							v22 = v42;
						} else {
							// Outside d1 & outside d3 ==> eliminate v1
							v1 = v4;
							v11 = v41;
							v12 = v42;
						}
					}
				}
			}
		}
	}
}