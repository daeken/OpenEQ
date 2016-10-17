using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace OpenEQ.GUI {
    public interface IWidget {
        Window ParentWindow { get; set; }
        void Render();
    }
}
