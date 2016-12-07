
namespace OpenEQ.FileConverter.Entities
{
    using System.Collections.Generic;

    public class AniTree
    {
        public int Bone;
        public Frame[] Frames;
        public IList<AniTree> Children;

        public AniTree(int idx, Frame[] frames, IList<AniTree> children)
        {
            Bone = idx;
            Frames = frames;
            Children = children;
        }
    }
}