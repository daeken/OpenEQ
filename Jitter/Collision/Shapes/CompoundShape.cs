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
    ///     A <see cref="Shape" /> representing a compoundShape consisting
    ///     of several 'sub' shapes.
    /// </summary>
    public class CompoundShape : Multishape {
		int currentShape;
		readonly List<int> currentSubShapes = new List<int>();

		JBBox mInternalBBox;

		Vector3 shifted;

        /// <summary>
        ///     Created a new instance of the CompountShape class.
        /// </summary>
        /// <param name="shapes">
        ///     The 'sub' shapes which should be added to this
        ///     class.
        /// </param>
        public CompoundShape(List<TransformedShape> shapes) {
			Shapes = new TransformedShape[shapes.Count];
			shapes.CopyTo(Shapes);

			if(!TestValidity())
				throw new ArgumentException("Multishapes are not supported!");

			UpdateShape();
		}

		public CompoundShape(TransformedShape[] shapes) {
			Shapes = new TransformedShape[shapes.Length];
			Array.Copy(shapes, Shapes, shapes.Length);

			if(!TestValidity())
				throw new ArgumentException("Multishapes are not supported!");

			UpdateShape();
		}


		internal CompoundShape() {
		}

        /// <summary>
        ///     An array conaining all 'sub' shapes and their transforms.
        /// </summary>
        public TransformedShape[] Shapes { get; set; }

		public Vector3 Shift => -1.0f * shifted;

		bool TestValidity() {
			for(var i = 0; i < Shapes.Length; i++)
				if(Shapes[i].Shape is Multishape)
					return false;

			return true;
		}

		public override void MakeHull(ref List<Vector3> triangleList, int generationThreshold) {
			var triangles = new List<Vector3>();

			for(var i = 0; i < Shapes.Length; i++) {
				Shapes[i].Shape.MakeHull(ref triangles, 4);
				for(var e = 0; e < triangles.Count; e++) {
					var pos = triangles[e];
					pos = pos.Transform(ref Shapes[i].orientation);
					triangleList.Add(pos + Shapes[i].position);
				}

				triangles.Clear();
			}
		}

        /// <summary>
        ///     Translate all subshapes in the way that the center of mass is
        ///     in (0,0,0)
        /// </summary>
        void DoShifting() {
			for(var i = 0; i < Shapes.Length; i++) shifted += Shapes[i].position;
			shifted *= 1.0f / Shapes.Length;

			for(var i = 0; i < Shapes.Length; i++) Shapes[i].position -= shifted;
		}

		public override void CalculateMassInertia() {
			inertia = JMatrix.Zero;
			mass = 0.0f;

			for(var i = 0; i < Shapes.Length; i++) {
				var currentInertia = Shapes[i].InverseOrientation * Shapes[i].Shape.Inertia * Shapes[i].Orientation;
				var p = Shapes[i].Position * -1.0f;
				var m = Shapes[i].Shape.Mass;

				currentInertia.M11 += m * (p.Y * p.Y + p.Z * p.Z);
				currentInertia.M22 += m * (p.X * p.X + p.Z * p.Z);
				currentInertia.M33 += m * (p.X * p.X + p.Y * p.Y);

				currentInertia.M12 += -p.X * p.Y * m;
				currentInertia.M21 += -p.X * p.Y * m;

				currentInertia.M31 += -p.X * p.Z * m;
				currentInertia.M13 += -p.X * p.Z * m;

				currentInertia.M32 += -p.Y * p.Z * m;
				currentInertia.M23 += -p.Y * p.Z * m;

				inertia += currentInertia;
				mass += m;
			}
		}

		protected override Multishape CreateWorkingClone() {
			var clone = new CompoundShape();
			clone.Shapes = Shapes;
			return clone;
		}


        /// <summary>
        ///     SupportMapping. Finds the point in the shape furthest away from the given direction.
        ///     Imagine a plane with a normal in the search direction. Now move the plane along the normal
        ///     until the plane does not intersect the shape. The last intersection point is the result.
        /// </summary>
        /// <param name="direction">The direction.</param>
        /// <param name="result">The result.</param>
        public override void SupportMapping(ref Vector3 direction, out Vector3 result) {
			direction.Transform(ref Shapes[currentShape].invOrientation, out result);
			Shapes[currentShape].Shape.SupportMapping(ref direction, out result);
			result.Transform(ref Shapes[currentShape].orientation, out result);
			result += Shapes[currentShape].position;
		}

        /// <summary>
        ///     Gets the axis aligned bounding box of the orientated shape. (Inlcuding all
        ///     'sub' shapes)
        /// </summary>
        /// <param name="orientation">The orientation of the shape.</param>
        /// <param name="box">The axis aligned bounding box of the shape.</param>
        public override void GetBoundingBox(ref JMatrix orientation, out JBBox box) {
			box.Min = mInternalBBox.Min;
			box.Max = mInternalBBox.Max;

			var localHalfExtents = 0.5f * (box.Max - box.Min);
			var localCenter = 0.5f * (box.Max + box.Min);

			var center = localCenter.Transform(ref orientation);

			JMath.Absolute(ref orientation, out var abs);
			var temp = localHalfExtents.Transform(ref abs);

			box.Max = center + temp;
			box.Min = center - temp;
		}

        /// <summary>
        ///     Sets the current shape. First <see cref="CompoundShape.Prepare" /> has to be called.
        ///     After SetCurrentShape the shape immitates another shape.
        /// </summary>
        /// <param name="index"></param>
        public override void SetCurrentShape(int index) {
			currentShape = currentSubShapes[index];
			Shapes[currentShape].Shape.SupportCenter(out geomCen);
			geomCen += Shapes[currentShape].Position;
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
			currentSubShapes.Clear();

			for(var i = 0; i < Shapes.Length; i++)
				if(Shapes[i].boundingBox.Contains(ref box) != JBBox.ContainmentType.Disjoint)
					currentSubShapes.Add(i);

			return currentSubShapes.Count;
		}

        /// <summary>
        /// </summary>
        /// <param name="rayOrigin"></param>
        /// <param name="rayEnd"></param>
        /// <returns></returns>
        public override int Prepare(ref Vector3 rayOrigin, ref Vector3 rayEnd) {
			var box = JBBox.SmallBox;

			box.AddPoint(ref rayOrigin);
			box.AddPoint(ref rayEnd);

			return Prepare(ref box);
		}


		public override void UpdateShape() {
			DoShifting();
			UpdateInternalBoundingBox();
			base.UpdateShape();
		}

		protected void UpdateInternalBoundingBox() {
			mInternalBBox.Min = new Vector3(float.MaxValue);
			mInternalBBox.Max = new Vector3(float.MinValue);

			for(var i = 0; i < Shapes.Length; i++) {
				Shapes[i].UpdateBoundingBox();

				JBBox.CreateMerged(ref mInternalBBox, ref Shapes[i].boundingBox, out mInternalBBox);
			}
		}

		#region public struct TransformedShape

        /// <summary>
        ///     Holds a 'sub' shape and it's transformation. This TransformedShape can
        ///     be added to the <see cref="CompoundShape" />
        /// </summary>
        public struct TransformedShape {
			internal Vector3 position;
			internal JMatrix orientation;
			internal JMatrix invOrientation;
			internal JBBox boundingBox;

            /// <summary>
            ///     The 'sub' shape.
            /// </summary>
            public Shape Shape { get; set; }

            /// <summary>
            ///     The position of a 'sub' shape
            /// </summary>
            public Vector3 Position {
				get => position;
				set {
					position = value;
					UpdateBoundingBox();
				}
			}

			public JBBox BoundingBox => boundingBox;

            /// <summary>
            ///     The inverse orientation of the 'sub' shape.
            /// </summary>
            public JMatrix InverseOrientation => invOrientation;

            /// <summary>
            ///     The orienation of the 'sub' shape.
            /// </summary>
            public JMatrix Orientation {
				get => orientation;
				set {
					orientation = value;
					JMatrix.Transpose(ref orientation, out invOrientation);
					UpdateBoundingBox();
				}
			}

			public void UpdateBoundingBox() {
				Shape.GetBoundingBox(ref orientation, out boundingBox);

				boundingBox.Min += position;
				boundingBox.Max += position;
			}

            /// <summary>
            ///     Creates a new instance of the TransformedShape struct.
            /// </summary>
            /// <param name="shape">The shape.</param>
            /// <param name="orientation">The orientation this shape should have.</param>
            /// <param name="position">The position this shape should have.</param>
            public TransformedShape(Shape shape, JMatrix orientation, Vector3 position) {
				this.position = position;
				this.orientation = orientation;
				JMatrix.Transpose(ref orientation, out invOrientation);
				Shape = shape;
				boundingBox = new JBBox();
				UpdateBoundingBox();
			}
		}

		#endregion
	}
}