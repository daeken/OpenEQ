using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace OpenEQ.Network {
    public enum SessionOp : ushort {
        Request = 0x0001, 
        Response = 0x0002, 
        Disconnect = 0x0005, 
        Stats = 0x0007, 
        Ack = 0x0015, 
        OutOfOrder = 0x0011, 
        Single = 0x0009, 
        Fragment = 0x000d, 
        Combined = 0x0003
    }
}
