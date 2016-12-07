
namespace OpenEQ.FileConverter.Entities
{
    public class ModelRef
    {
        public string _name;
        public FragRef[] skeleton;

        public ModelRef(string name, FragRef[] s)
        {
            _name = name;
            skeleton = s;
        }
    }
}