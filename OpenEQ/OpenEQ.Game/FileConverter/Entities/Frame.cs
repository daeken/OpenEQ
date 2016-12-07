
namespace OpenEQ.FileConverter.Entities
{
    using GlmNet;

    public class Frame
    {
        public string _name;
        public vec3 position;
        public vec4 rotation;

        public Frame(string name, float shiftx, float shifty, float shiftz, float rotx, float roty, float rotz, float rotw)
        {
            _name = name;
            position = new vec3(shiftx, shifty, shiftz);
            rotation = new vec4(rotx, roty, rotz, rotw);
        }
    }
}