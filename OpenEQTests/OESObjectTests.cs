namespace OpenEQTests
{
    public class OESObjectTests
    {
        OpenEQ.Common.OESObject oESObject;
        String testObjectName = "testObjectName";
        [SetUp]
        public void Setup()
        {
            oESObject = new OpenEQ.Common.OESObject() { Name = testObjectName };
        }

        [Test]
        public void Test1()
        {
            Assert.That(testObjectName, Is.EqualTo(testObjectName.ToString()));
        }
    }
}