
using System.Collections.Generic;

namespace OpenEQ.FileConverter.Entities
{
    public class TexRef
    {
        public string _name;
        public int SaneFlags;
        public FragRef[] Value;

        public TexRef(string name, int saneFlags, FragRef[] v)
        {
            _name = name;
            SaneFlags = saneFlags;
            Value = v;
        }

        public IEnumerable<string> Resolve()
        {
            foreach (var fr in Value)
            {
                foreach (var s in fr.Resolve())
                {
                    yield return s;
                }
            }
        }
    }
}