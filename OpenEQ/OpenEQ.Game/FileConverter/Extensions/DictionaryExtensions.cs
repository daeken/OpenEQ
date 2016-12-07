
namespace OpenEQ.FileConverter.Extensions
{
    using System.Collections.Generic;
    using System.Linq;

    public static class DictionaryExtensions
    {
        public static IDictionary<TKey, TValue> Merge<TKey, TValue>(this IDictionary<TKey, TValue> dictA,
            IDictionary<TKey, TValue> dictB)
            where TValue : class
        {
            return dictA.Keys.Union(dictB.Keys).ToDictionary(k => k, k => dictA.ContainsKey(k) ? dictA[k] : dictB[k]);
        }
    }
}