
namespace OpenEQ.FileConverter.Entities
{
    public class ObjectLocation
    {
        public string _name;
        public float[] Position;
        public float[] Rotation;
        public float[] Scale;
        public int NameOffset;

        public ObjectLocation(string name, float[] position, float[] rotation, float[] scale, int nameOffset)
        {
            _name = name;
            Position = position;
            Rotation = rotation;
            Scale = scale;
            NameOffset = nameOffset;
        }
    }
}