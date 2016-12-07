
namespace OpenEQ.Chat
{
    using System;
    using System.Text;

    public class ChatServer
    {
        public string ServerAddress { get; }
        public int ServerPort { get; }
        public string ShortName { get; }
        public string CharName { get; }
        public char ConnectionType { get; }
        public string MailKey { get; }

        public ChatServer(byte[] chatData)
        {
            var firstSplit = Encoding.UTF8.GetString(chatData).Split(',');
            var secondSplit = firstSplit[2].Split('.');

            ServerAddress = firstSplit[0];
            ServerPort = Convert.ToInt32(firstSplit[1]);
            ShortName = secondSplit[0];
            CharName = secondSplit[1];
            ConnectionType = firstSplit[3][0];
            MailKey = firstSplit[3].Substring(1).TrimEnd(char.MinValue);
        }
    }
}