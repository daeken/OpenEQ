
namespace OpenEQ.Classes
{
    using System;

    public static class ClassTypesExtensions
    {
        public static string GetClassName(this ClassTypes classType)
        {
            return Enum.GetName(typeof(ClassTypes), classType);
        }
    }
}