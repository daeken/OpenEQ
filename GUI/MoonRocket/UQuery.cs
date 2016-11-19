using System.Collections.Generic;
using LibRocketNet;
using MoonSharp.Interpreter;
using static System.Console;
using System.Linq;
using HtmlAgilityPack;
using System;

namespace OpenEQ.GUI.MoonRocket {
    public class UQDataSource : DataSource {
        List<Dictionary<string, string>> table;

        public UQDataSource(string name, List<Dictionary<string, string>> table) : base(name) {
            this.table = table;
        }

        public override int GetNumRows(string name) {
            return table.Count();
        }

        public override List<string> GetRow(string name, int row, List<string> columns) {
            return columns.Where(col => table[row].ContainsKey(col)).Select(col => table[row][col]).ToList();
        }
    }

    [MoonSharpUserData]
    public class StyleProxy {
        UQuery uq;

        public string this[string key] {
            get {
                return uq.GetProperty(key);
            }
            set {
                uq.SetProperty(key, value);
            }
        }

        public StyleProxy(UQuery uq) {
            this.uq = uq;
        }
    }

    [MoonSharpUserData]
    public class UQuery {
        public List<Element> Elements;

        public Element this[int idx] {
            get {
                return Elements[idx];
            }
        }

        public UQuery Children {
            get {
                return new UQuery(Elements.Select(elem => elem.Children).SelectMany(x => x));
            }
        }

        public UQuery Parent {
            get {
                return new UQuery(Elements.Select(elem => elem.ParentNode).Where(elem => elem != null));
            }
        }

        public string Value {
            get {
                if(Elements.Count == 0) return "";
                return Elements[0].GetAttributeString("value", "");
            }
            set {
                foreach(var elem in Elements)
                    elem.SetAttribute("value", value);
            }
        }

        public string Text {
            get {
                if(Elements.Count == 0) return "";
                return HtmlEntity.DeEntitize(Elements[0].InnerRml);
            }
            set {
                var enc = HtmlEntity.Entitize(value);
                foreach(var elem in Elements)
                    elem.InnerRml = enc;
            }
        }

        public string this[string key] {
            get {
                return Elements[0].GetAttributeString(key, "");
            }
            set {
                foreach(var elem in Elements)
                    elem.SetAttribute(key, value);
            }
        }

        public string GetProperty(string key) {
            if(Elements.Count == 0)
                return "";
            return Elements[0].GetPropertyString(key);
        }

        public void SetProperty(string key, string value) {
            if(value == null || value == "")
                foreach(var elem in Elements)
                    elem.RemoveProperty(key);
            else
                foreach(var elem in Elements)
                    elem.SetProperty(key, value);
        }

        StyleProxy style;
        public StyleProxy Style {
            get {
                if(style == null)
                    style = new StyleProxy(this);
                return style;
            }
        }

        public UQuery(Element element) : this(new List<Element>() { element }) {}
        public UQuery(IEnumerable<Element> elements) : this(elements.ToList()) {}

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
            return new UQuery(Elements.Select(elem => elem.AppendHtmlChild(html)).SelectMany(i => i));
        }
        public UQuery Prepend(string html) {
            WriteLine("Prepending elements is currently non-functional.");
            html = $"<body>{html}</body>";
            return new UQuery(Elements.Select(elem => elem.PrependHtmlChild(html)).SelectMany(i => i));
        }

        public UQuery Remove(string selector = "") {
            var uq = selector != "" ? Select(selector) : this;

            foreach(var elem in uq.Elements) {
                elem.ParentNode.RemoveChild(elem);
                Elements.Remove(elem);
            }

            return this;
        }

        public UQuery Bind(string evt, DynValue callback) {
            var cb = callback.Function.GetDelegate();

            foreach(var element in Elements)
                element.Bind(evt, cb);

            return this;
        }

        public UQuery Data(Table rows) {
            var dicts = new List<Dictionary<string, string>>();
            foreach(var row in rows.Values) {
                var cur = new Dictionary<string, string>();
                dicts.Add(cur);
                foreach(var cell in row.Table.Pairs)
                    cur[cell.Key.String] = cell.Value.CastToString();
                if(dicts.Count == 10)
                    break;
            }

            foreach(var elem in Elements) {
                var dsname = "testing";//$"{elem.GetHashCode():X08}_{GetHashCode():X08}";
                var ds = new UQDataSource(dsname, dicts);
                elem.SetDataSource($"{dsname}.table");
            }
            
            return this;
        }

        public UQuery Show() {
            Style["display"] = "";
            return this;
        }
        public UQuery Hide() {
            Style["display"] = "none";
            return this;
        }

        public override string ToString() {
            var ret = "UQuery[";
            if(Elements.Count == 1)
                ret += FormatElement(Elements[0]);
            else if(Elements.Count > 1)
                ret += string.Join(",\n", Elements.Select(elem => "\t" + FormatElement(elem)));
            return ret + ']';
        }

        string FormatElement(Element elem) {
            var ret = elem.TagName;
            ret += elem.ClassNames;
            if(elem.Id != "")
                ret += "#" + elem.Id;
            return ret;
        }
    }
}
