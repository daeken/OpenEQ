
namespace OpenEQ.FileConverter.Entities
{
    using System.Collections.Generic;

    public class SkelPierceTrackSet
    {
        public string _name;
        public SkelPierceTrack[] Tracks;
        public List<FragRef> Meshes;

        public SkelPierceTrackSet(uint trackCount, string name)
        {
            _name = name;
            Tracks = new SkelPierceTrack[trackCount];
        }
    }
}