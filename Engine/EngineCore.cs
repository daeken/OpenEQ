using System;
using System.Collections;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Numerics;
using CollisionManager;
using Noesis;
using OpenTK;
using OpenTK.Graphics;
using OpenTK.Graphics.OpenGL4;
using OpenTK.Input;
using PrettyPrinter;
using static OpenEQ.Engine.Globals;
using Key = OpenTK.Input.Key;
using Mouse = OpenTK.Input.Mouse;
using MouseButton = OpenTK.Input.MouseButton;
using Vector2 = System.Numerics.Vector2;
using Vector3 = System.Numerics.Vector3;

namespace OpenEQ.Engine {
	public partial class EngineCore : GameWindow {
		bool DeferredEnabled;

		readonly List<Model> Models = new List<Model>();
		readonly List<AniModelInstance> AniModels = new List<AniModelInstance>();
		readonly List<double> FrameTimes = new List<double>();
		readonly List<PointLight> Lights = new List<PointLight>();

		public readonly View View;

		public double FPS => FrameTimes.Count == 0 ? 0 : 1 / (FrameTimes.Sum() / FrameTimes.Count);

		Matrix4x4 ProjectionView;

		bool MouseLooking;
		(double, double) MouseBeforeLook;

		(double X, double Y) MousePosition {
			get {
				var state = Mouse.GetCursorState();
				return (state.X, state.Y);
			}
			set => Mouse.SetPosition(value.X, value.Y);
		}

		Vector2 MouseDelta {
			get {
				var state = Mouse.GetState();
				return vec2(state.X, state.Y);
			}
		}

		Vector2 LastMouseDelta;

		public EngineCore() : base(
			1280, 720, new GraphicsMode(new ColorFormat(8, 8, 8, 8), 24, 8), "OpenEQ",
			GameWindowFlags.Default, DisplayDevice.Default, 4, 1, GraphicsContextFlags.ForwardCompatible
		) {
			VSync = VSyncMode.Off;
			Log.SetLogCallback((level, channel, message) => {
				if(channel != "") return;
				// [TRACE] [DEBUG] [INFO] [WARNING] [ERROR]
				string[] prefixes = { "T", "D", "I", "W", "E" };
				var prefix = (int) level < prefixes.Length ? prefixes[(int) level] : " ";
				Console.WriteLine("[NOESIS/" + prefix + "] " + message);
			});
			GUI.Init();
			var xaml = (Grid) GUI.ParseXaml(@"
				<Grid xmlns=""http://schemas.microsoft.com/winfx/2006/xaml/presentation"" xmlns:x=""http://schemas.microsoft.com/winfx/2006/xaml"" x:Name=""_Root"">
				</Grid>
			");
			View = GUI.CreateView(xaml);
			View.SetIsPPAAEnabled(true);

			Load += (_, __) => View.Renderer.Init(new RenderDeviceGL());

			MouseMove += (_, e) => {
				//MousePosition
				if(!MouseLooking)
					View.MouseMove(e.X, e.Y);
			};
			MouseDown += (_, e) => UpdateMouseButton(e.Button, true, e.X, e.Y);
			MouseUp += (_, e) => UpdateMouseButton(e.Button, false, e.X, e.Y);
			MouseWheel += (_, e) => View.MouseWheel(e.X, e.Y, e.Delta);

			KeyPress += (_, e) => {
				//$"{(int)e.KeyChar:X}".Print();
				View.Char(e.KeyChar);
			};

			Resize += (_, e) => {
				View.SetSize(Width, Height);
				ProjectionMat =
					Matrix4x4.CreatePerspectiveFieldOfView(45 * (MathF.PI / 180), (float) Width / Height, 1, 5000);
			};
		}
		
		public void AddLight(Vector3 pos, float radius, float attenuation, Vector3 color) =>
			Lights.Add(new PointLight(pos, radius, attenuation, color));

		public void Add(Model model) => Models.Add(model);
		public void Add(AniModelInstance modelInstance) => AniModels.Add(modelInstance);

		public void Start() {
			var ot = new List<Triangle>();
			Console.WriteLine("Building mesh for physics");
			foreach(var model in Models) {
				if(!model.IsFixed) continue;
				foreach(var mesh in model.Meshes) {
					if(!mesh.IsCollidable) continue;
					ot.AddRange(mesh.PhysicsMesh);
				}
			}

			Console.WriteLine($"Building octree for {ot.Count} triangles");
			Collider = new CollisionHelper(new Octree(new CollisionManager.Mesh(ot), 250));
			Console.WriteLine("Built octree");

			//Debugging.Add(new Wireframe(ot));

			Run();
		}

		void UpdateMouseButton(MouseButton button, bool state, int x, int y) {
			switch(button) {
				case MouseButton.Left:
					if(state && View.MouseButtonDown(x, y, Noesis.MouseButton.Left)) { }
					else if(!state && !View.MouseButtonUp(x, y, Noesis.MouseButton.Left)) { }
					else
						View.Content.Keyboard.Focus(null);

					break;
				case MouseButton.Right:
					if(state && View.MouseButtonDown(x, y, Noesis.MouseButton.Right)) { }
					else if(!state && View.MouseButtonUp(x, y, Noesis.MouseButton.Right)) { }
					else {
						if(state) {
							MouseLooking = true;
							MousePosition.Print();
							MouseBeforeLook = MousePosition;
							CursorVisible = false;
							LastMouseDelta = MouseDelta;
						}
						else {
							MouseLooking = false;
							CursorVisible = true;
							MousePosition.Print();
							MousePosition = MouseBeforeLook;
							MousePosition.Print();
						}
					}

					break;
			}
		}

		readonly Dictionary<Key, bool> KeyState = new Dictionary<Key, bool>();

		Noesis.Key MapKey(Key key) => key switch {
			Key.Space => Noesis.Key.Space, 
			Key.Tab => Noesis.Key.Tab, 
			Key.BackSpace => Noesis.Key.Back, 
			_ => Noesis.Key.NoName
		};
		
		protected override void OnKeyDown(KeyboardKeyEventArgs e) {
			if(View.KeyDown(MapKey(e.Key))) return;
			switch(e.Key) {
				case Key.L:
					DeferredEnabled = !DeferredEnabled;
					break;
				case Key.Space:
					if(Camera.OnGround)
						Camera.FallingVelocity = -50;
					break;
				case Key.P:
					PhysicsEnabled = !PhysicsEnabled;
					break;
				default:
					KeyState[e.Key] = true;
					break;
			}
		}

		protected override void OnKeyUp(KeyboardKeyEventArgs e) {
			if(!View.KeyUp(MapKey(e.Key)))
				KeyState.Remove(e.Key);
		}

		protected override void OnUpdateFrame(FrameEventArgs e) {
			var movement = vec3();
			var movescale = KeyState.Keys.Contains(Key.ShiftLeft) ? 250 : 75;
			var pitchscale = 1.25f;
			var yawscale = 1.25f;
			foreach(var key in KeyState.Keys)
				switch(key) {
					case Key.W:
						movement += vec3(0, (float) e.Time * movescale, 0);
						break;
					case Key.S:
						movement += vec3(0, (float) -e.Time * movescale, 0);
						break;
					case Key.A:
						movement += vec3((float) -e.Time * movescale, 0, 0);
						break;
					case Key.D:
						movement += vec3((float) e.Time * movescale, 0, 0);
						break;
					case Key.Up:
						Camera.Look((float) e.Time * yawscale, 0);
						break;
					case Key.Down:
						Camera.Look((float) -e.Time * yawscale, 0);
						break;
					case Key.Left:
						Camera.Look(0, (float) e.Time * pitchscale);
						break;
					case Key.Right:
						Camera.Look(0, (float) -e.Time * pitchscale);
						break;
					case Key.Home:
						Camera.Position.Z = 1000;
						break;
					case Key.Escape:
					case Key.Tilde:
						Exit();
						break;
				}
			if(movement.Length() > 0)
				Camera.Move(movement);
			var mdelta = MouseDelta - LastMouseDelta;
			LastMouseDelta = MouseDelta;
			if(MouseLooking && (mdelta.X != 0 || mdelta.Y != 0)) {
				var oscale = -0.005f;
				Camera.Look(mdelta.Y * pitchscale * oscale, mdelta.X * yawscale * oscale);
			}

			Camera.Update((float) e.Time);

			base.OnUpdateFrame(e);
		}

		protected override void OnRenderFrame(FrameEventArgs e) {
			FrameTime = Time;
			if(FrameTimes.Count == 200)
				FrameTimes.RemoveAt(0);
			FrameTimes.Add(e.Time);

			ProjectionView = FpsCamera.Matrix * ProjectionMat;

			GL.Disable(EnableCap.StencilTest);
			GL.Enable(EnableCap.CullFace);
			GL.Disable(EnableCap.Blend);
			GL.Enable(EnableCap.DepthTest);
			GL.DepthMask(true);

			if(DeferredEnabled) {
				SetupDeferredPathway();
				NoProfile("Deferred render", RenderDeferredPathway);
			}

			NoProfile("Forward render", () => {
				if(!DeferredEnabled) {
					GL.Viewport(0, 0, Width, Height);
					GL.ClearStencil(0);
					GL.ClearColor(0, 0, 0, 1);
					GL.Clear(ClearBufferMask.ColorBufferBit | ClearBufferMask.DepthBufferBit | ClearBufferMask.StencilBufferBit);
		
					Models.ForEach(model => model.Draw(ProjectionView, forward: false));
					AniModels.ForEach(model => model.Draw(ProjectionView, forward: false));
				}
				GL.Enable(EnableCap.Blend);
				GL.BlendFunc(BlendingFactor.SrcAlpha, BlendingFactor.OneMinusSrcAlpha);
				GL.Enable(EnableCap.DepthTest);
				GL.ActiveTexture(TextureUnit.Texture0);
				GL.DepthMask(false);
				Models.ForEach(model => model.Draw(ProjectionView, forward: true));
				AniModels.ForEach(model => model.Draw(ProjectionView, forward: true));
				GL.DepthMask(true);
				GL.Finish();
			});
			
			Debugging.Draw(ProjectionView);

			View.Update(Time);
			View.Renderer.UpdateRenderTree();
			View.Renderer.RenderOffscreen();
			GL.BindFramebuffer(FramebufferTarget.Framebuffer, 0);
			View.Renderer.Render();

			SwapBuffers();
		}
	}
}