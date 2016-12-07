
namespace OpenEQ.FileConverter.Entities
{
    public class SkelPierceTrack
    {
        public string Name;
        public FragRef[] pierceTrack;
        public uint flags;
        public int[] NextPieces;

        public SkelPierceTrack()
        {

        }

        public SkelPierceTrack(string name, FragRef[] t)
        {
            Name = name;
            pierceTrack = t;
        }
    }
}