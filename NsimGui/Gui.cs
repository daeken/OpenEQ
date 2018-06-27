using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Numerics;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using ImGuiNET;

namespace OpenEQ.NsimGui {
	public class Gui {
		static IO IO => ImGui.GetIO();

		readonly IGuiRenderer Renderer;
		
		public Vector2 Dimensions {
			get => IO.DisplaySize;
			set => IO.DisplaySize = value;
		}
		
		public Vector2 Scale {
			get => IO.DisplayFramebufferScale;
			set => IO.DisplaySize = value;
		}
		
		public unsafe Gui(IGuiRenderer renderer) {
			Renderer = renderer;
			IO.FontAtlas.AddDefaultFont();
			var fontTex = IO.FontAtlas.GetTexDataAsAlpha8();
			IO.FontAtlas.SetTexID(Renderer.CreateTexture(
				TextureFormat.Alpha, fontTex.Width, fontTex.Height, 
				PointerToArray<byte>(fontTex.Pixels, fontTex.Width * fontTex.Height)));
		}

		public void Render(float deltaTime) {
			IO.DeltaTime = deltaTime;
			ImGui.NewFrame();

			ImGui.BeginWindow("Foo!");
			ImGui.Button("ASsdf");
			ImGui.EndWindow();
			
			ImGui.Render();
			Renderer.Draw(BuildDrawCommandSets());
		}

		unsafe T[] PointerToArray<T>(void* ptr, int count) where T : struct {
			var sptr = (byte*) ptr;
			var arr = new T[count];
			var size = Marshal.SizeOf<T>();
			for(var i = 0; i < count; ++i) {
				arr[i] = Unsafe.Read<T>(sptr);
				sptr += size;
			}
			return arr;
		}

		unsafe IReadOnlyList<DrawCommandSet> BuildDrawCommandSets() {
			var data = ImGui.GetDrawData();
			var csets = new List<DrawCommandSet>();

			for(var n = 0; n < data->CmdListsCount; ++n) {
				var cmdList = data->CmdLists[n];
				var commands = new List<DrawCommand>();
				var ioff = 0U;
				for(var i = 0; i < cmdList->CmdBuffer.Size; ++i) {
					var cmd = ((DrawCmd*) cmdList->CmdBuffer.Data)[i];
					Debug.Assert(cmd.UserCallback == IntPtr.Zero);

					commands.Add(new DrawCommand {
						TextureId = (int) cmd.TextureId, 
						Scissor = (
							cmd.ClipRect.X, IO.DisplaySize.Y- cmd.ClipRect.W, 
							cmd.ClipRect.Z - cmd.ClipRect.X, cmd.ClipRect.W - cmd.ClipRect.Y
						), 
						IndexOffset = ioff, ElementCount = cmd.ElemCount
					});
					ioff += cmd.ElemCount;
				}
				csets.Add(new DrawCommandSet {
					VBufferData = PointerToArray<byte>(cmdList->VtxBuffer.Data, cmdList->VtxBuffer.Size * sizeof(DrawVert)), 
					IBufferData = PointerToArray<ushort>(cmdList->IdxBuffer.Data, cmdList->IdxBuffer.Size), 
					Commands = commands
				});
			}

			return csets;
		}
	}
}