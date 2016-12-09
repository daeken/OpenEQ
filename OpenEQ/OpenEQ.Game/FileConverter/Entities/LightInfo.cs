
namespace OpenEQ.FileConverter.Entities
{
    public class LightInfo
    {
        public string _name;
        public FragRef[] lref;
        public uint flags;
        public float[] pos;
        public float radius;

        public LightInfo(string name, FragRef[] lr, uint f, float[] p, float r)
        {
            _name = name;
            lref = lr;
            flags = f;
            pos = p;
            radius = r;
        }
    }
}