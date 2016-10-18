using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace OpenEQ.GUI {
    public abstract class BaseWidget : IWidget {
        public bool Visible = true;
        public Window ParentWindow { get; set; }

        public abstract void Render();
    }
}
