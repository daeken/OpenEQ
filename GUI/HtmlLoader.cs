using LibRocketNet;
using HtmlAgilityPack;
using System.IO;

namespace OpenEQ.GUI {
    public static class HtmlLoader {
        public static ElementDocument LoadHtmlDocument(this Context ctx, string path) {
            HtmlNode.ElementsFlags.Remove("script");
            HtmlNode.ElementsFlags.Remove("style");
            var doc = new HtmlDocument();
            doc.Load(path);
            doc.OptionOutputAsXml = true;
            using(var ms = new MemoryStream()) {
                doc.Save(ms);
                ms.Position = 0;
                var sr = new StreamReader(ms);
                var html = sr.ReadToEnd();
                return ctx.LoadDocumentFromMemory(html);
            }
        }
    }
}
