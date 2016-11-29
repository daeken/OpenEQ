
using System;
using System.Collections.Generic;

namespace OpenEQ.FileConverter.Entities
{
    public class FragRef
    {
        public int Id;
        public string Name;
        public object Value;

        public FragRef(int id = -1, string name = "", object value = null)
        {
            Id = id;
            Name = name;
            Value = value;
        }

        public string[] Resolve()
        {
            var outList = new List<string>();// {Value};
            var tmp = Value;

            if (Value is string[])
                return (string[])Value;
//            if (null == Value) return outList;

            // Clear the list because we're not at the end.
            //outList.Clear();

            if (tmp is Tuple<int, string, uint, object>)
            {
                tmp = ((Tuple<int, string, uint, object>) tmp).Item4;
            }

            if (tmp is FragRef[])
            {
                var iter = (FragRef[]) tmp;

                for (var i = 0; i < iter.Length; i++)
                {
                    outList.AddRange(iter[i].Resolve());
                }

                return outList.ToArray();
            }

            if (tmp is TexRef)
            {
                outList.AddRange(((TexRef) tmp).Resolve());
            }

            return outList.ToArray();
        }
    }
}