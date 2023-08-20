using System.Text.RegularExpressions;
using System.Xml.Linq;

namespace OpenEQTests
{
    public class OESObjectTests
    {
        OpenEQ.Common.OESObject oESObject;
        String testObjectName = "Rackspace";
        [SetUp]
        public void Setup()
        {
            oESObject = new OpenEQ.Common.OESObject(testObjectName);
        }

        [Test]
        public void Test1()
        {
            Assert.That(testObjectName, Is.Not.EqualTo(oESObject.ToString()));
        }

        [Test]
        public void Test2()
        {
            OpenEQ.Common.OESObject oESOBJ = new OpenEQ.Common.OESObject();
            Assert.That(oESOBJ.Name, Is.Not.EqualTo(oESOBJ.ToString()));
        }

        [Test]
        public void Test3()
        {
            Regex regex = new Regex(@"OESObject\(Name=(?<name>\S+)\)");
            Match m = regex.Match(oESObject.ToString());
            Assert.That(oESObject.Name, Is.EqualTo(m.Groups["name"].Value));
        }
    }
}