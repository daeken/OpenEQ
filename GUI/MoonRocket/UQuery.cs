using System.Collections.Generic;
using LibRocketNet;
using MoonSharp.Interpreter;
using static System.Console;
using System.Linq;

namespace OpenEQ.GUI.MoonRocket {
    [MoonSharpUserData]
    public class UQuery {
        public List<Element> Elements;

        public Element this[int idx] {
            get {
                return Elements[idx];
            }
        }

        public string Value {
            get {
                if(Elements.Count == 0) return "";
                return Elements[0].GetAttributeString("value", "");
            }
            set {
                if(Elements.Count > 0)
                    foreach(var elem in Elements)
                        elem.SetAttribute("value", value);
            }
        }

        public UQuery(Element element) {
            Elements = new List<Element>() { element };
        }

        public UQuery(List<Element> elements) {
            Elements = elements;
        }

        public UQuery Global(DynValue arg) {
            if(arg.Type == DataType.String)
                return Select(arg.ToObject<string>());
            return new UQuery(new List<Element>() { arg.ToObject<Element>() });
        }

        public UQuery Select(string selector) {
            var elist = new List<Element>();

            if(selector[0] == '#') {
                selector = selector.Substring(1);
                elist.AddRange(Elements.Select(elem => elem.GetElementById(selector)).Where(elem => elem != null));
            } else if(selector[0] == '.') {
                selector = selector.Substring(1);
                elist.AddRange(Elements.Select(elem => elem.GetElementsByClassName(selector)).SelectMany(i => i));
            } else {
                elist.AddRange(Elements.Select(elem => elem.GetElementsByTagName(selector)).SelectMany(i => i));
            }

            return new UQuery(elist);
        }

        public UQuery Append(string html) {
            html = $"<body>{html}</body>";
            return new UQuery(Elements.Select(elem => elem.AppendHtmlChild(html)).SelectMany(i => i).ToList());
        }

        public UQuery Bind(string evt, DynValue callback) {
            var cb = callback.Function.GetDelegate();

            foreach(var element in Elements)
                element.Bind(evt, cb);

            return this;
        }
    }
}
