
using System;
using System.Collections.Generic;
using System.Linq;

namespace OpenEQ.FileConverter.Entities
{
    public class FragRef
    {
        public int Id;
        public string Name;
        public dynamic Value;

        //public dynamic this[int i]
        //{
        //    get
        //    {
        //        return ResolveInternal(this).ToList()[i];
        //    }
        //}

        public FragRef(int id = -1, string name = "", object value = null)
        {
            Id = id;
            Name = name;
            Value = value;
        }

        private static IEnumerable<dynamic> ResolveInternal(dynamic obj)
        {
            yield return obj;
            // At the end of the list.  Return ourselves.
            //            if (Value == null)
            //              yield return this;
            //        else
            if (obj is string[])
            {
                yield return obj;
            }

            if (obj is FragRef)
            {
                yield return obj;
            }

            if (obj is FragRef[])
            {
                var iter = (FragRef[]) obj;
                foreach (var fr in iter)
                {
                    yield return ResolveInternal(fr);
                }
            }

            if (obj is TexRef)
            {
                yield return ResolveInternal(obj);
            }
        }

        public IEnumerable<string> Resolve()
        {
            var outList = new List<string>();// {Value};
            var tmp = Value;

            if (Value is string[])
            {
                foreach (var s in (string[]) Value)
                {
                    yield return s;
                }
            }

            if (tmp is Tuple<int, string, uint, object>)
            {
                tmp = ((Tuple<int, string, uint, object>) tmp).Item4;
            }

            if (tmp is FragRef[])
            {
                var iter = (FragRef[]) tmp;

                foreach (var fr in iter)
                {
                    foreach (var s in fr.Resolve())
                    {
                        yield return s;
                    }
                }
            }

            if (tmp is TexRef)
            {
                foreach (var s in ((TexRef) tmp).Resolve())
                {
                    yield return s;
                }
            }
        }
    }
}