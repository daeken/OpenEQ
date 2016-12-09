
namespace OpenEQ.FileConverter.Extensions
{
    using System.Collections.Generic;

    public static class IEnumerableExtensions
    {
        internal static IEnumerable<float> InterleaveAndFlattenEqualLength(this IList<GlmNet.vec3> first,
            IList<GlmNet.vec3> second, IList<float[]> third)
        {
            for (var i = 0; i < first.Count; i++)
            {
                yield return first[i].x;
                yield return first[i].y;
                yield return first[i].z;
                yield return second[i].x;
                yield return second[i].y;
                yield return second[i].z;
                yield return third[i][0];
                yield return third[i][1];
            }
        }
    }
}