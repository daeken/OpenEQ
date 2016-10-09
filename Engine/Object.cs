using System.Collections.Generic;

namespace OpenEQ.Engine {
    public class Object {
        List<Mesh> meshes;
        public Object() {
            meshes = new List<Mesh>();
        }
        public void AddMesh(Mesh mesh) {
            meshes.Add(mesh);
        }

        public void Draw() {
            foreach(var mesh in meshes)
                mesh.Draw();
        }
    }
}