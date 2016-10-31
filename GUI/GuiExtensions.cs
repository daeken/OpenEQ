using LibRocketNet;
using HtmlAgilityPack;
using System.IO;
using System.Collections.Generic;
using static System.Console;

namespace OpenEQ.GUI {
    public static class GuiExtensions {
        public static string HtmlToXml(string html) {
            HtmlNode.ElementsFlags.Remove("script");
            HtmlNode.ElementsFlags.Remove("style");
            HtmlNode.ElementsFlags.Remove("form");
            var doc = new HtmlDocument();
            doc.LoadHtml(html);
            doc.OptionOutputAsXml = true;
            using(var ms = new MemoryStream()) {
                doc.Save(ms);
                ms.Position = 0;
                var sr = new StreamReader(ms);
                var xml = sr.ReadToEnd();
                return xml;
            }
        }

        public static ElementDocument LoadHtmlDocument(this Context ctx, string path) {
            return ctx.LoadDocumentFromMemory(HtmlToXml(File.ReadAllText(path)));
        }

        public static List<Element> AppendHtmlChild(this Element elem, string html) {
            return elem.AppendChild(HtmlToXml(html));
        }
        public static List<Element> PrependHtmlChild(this Element elem, string html) {
            return elem.AppendChild(HtmlToXml(html));
        }
    }
}
