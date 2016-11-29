
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

        public string[] Resolve()
        {
            var outList = new List<string>();// {Value};

//            if (null == Value)
//                return (string[]) Value;//outList;

            // Don't need the value.
            outList.Clear();
            for (var i = 0; i < Value.Length; i++)
            {
                outList.AddRange(Value[i].Resolve());
            }

            return outList.ToArray();
        }
    }
}