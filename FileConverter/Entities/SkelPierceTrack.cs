
namespace OpenEQ.FileConverter.Entities
{
    public class SkelPierceTrack
    {
        public string _name;
        public FragRef[] pierceTrack;
        public uint flags;
        public int[] NextPieces;

        public SkelPierceTrack()
        {

        }

        public SkelPierceTrack(string name, FragRef[] t)
        {
            _name = name;
            pierceTrack = t;
        }
    }
}