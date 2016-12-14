using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using OpenEQ.Chat;
using OpenEQ.Classes;
using SiliconStudio.Core.Mathematics;
using SiliconStudio.Xenko.Input;
using SiliconStudio.Xenko.Engine;
using OpenEQ.Network;
using SiliconStudio.Xenko.UI;
using SiliconStudio.Xenko.UI.Controls;
using SiliconStudio.Xenko.UI.Panels;

namespace OpenEQ {
    public class WorldScript : UIScript {
        internal WorldStream world;

#pragma warning disable 0649
//        [PageElement] Button loginButton;
//        [PageElement] EditText username, password;
//        [PageElement] TextBlock status;
//        [PageElement] UniformGrid authSection;
//        [PageElement] StackPanel serverListSection;
        [PageElement] Button buttonCreateCharacter;
        [PageElement] Grid charGrid;
        [PageElement] TextBlock charNameHeader;
#pragma warning restore 0649

        public override void Setup() {
            string charname = null;
            world.CharacterCreateNameApproval += (sender, approvalState) =>
            {
                if (1 == approvalState)
                {
                    // approved.  Send charactercreate and EnterWorld.
                    world.SendCharacterCreate(new CharCreate
                    {
                        Class_ = 1,
                        Haircolor = 255,
                        BeardColor = 255,
                        Beard = 255,
                        Gender = 0,
                        Race = 2,
                        StartZone = 29,
                        HairStyle = 255,
                        Deity = 211,
                        STR = 113,
                        STA = 130,
                        AGI = 87,
                        DEX = 70,
                        WIS = 70,
                        INT = 60,
                        CHA = 55,
                        Face = 0,
                        EyeColor1 = 9,
                        EyeColor2 = 9,
                        DrakkinHeritage = 1,
                        DrakkinTattoo = 0,
                        DrakkinDetails = 0,
                        Tutorial = 0
                    });

                    world.SendEnterWorld(new EnterWorld
                    {
                        Name = charname,
                        GoHome = false,
                        Tutorial = false
                    });
                }
                else
                {
                    // Denied.  Let them try a different name.
                }
            };

            buttonCreateCharacter.Click += (sender, args) =>
            {
                charname = "JimTomShiner";

                //Console.WriteLine($"1. Barbarian, Male, Warrior, Name is {charname}");
                //Console.WriteLine("Enter number of the character type you want and press Enter.");
                //var selection = Console.ReadLine();

                // Send char create packet with defaults.
                world.SendNameApproval(new NameApproval
                {
                    Name = charname,
                    Class = 3,
                    Race = 1,
                    Unknown = 214
                });
            };

            world.CharacterList += (sender, chars) =>
            {
                var i = 0;
                foreach (var character in chars)
                {
                    charGrid.RowDefinitions.Add(new StripDefinition(StripType.Fixed, 25));
                    var namefield = new TextBlock
                    {
                        Text = character.Name,
                        Font = charNameHeader.Font,
                        TextSize = charNameHeader.TextSize,
                        TextColor = charNameHeader.TextColor
                    };
                    namefield.SetGridColumn(0);
                    namefield.SetGridRow(i);
                    charGrid.Children.Add(namefield);

                    var classfield = new TextBlock
                    {
                        Text = ((ClassTypes)character.Class).GetClassName(),
                        Font = charNameHeader.Font,
                        TextSize = charNameHeader.TextSize,
                        TextColor = charNameHeader.TextColor
                    };
                    classfield.SetGridColumn(1);
                    classfield.SetGridRow(i);
                    charGrid.Children.Add(classfield);

                    var levelfield = new TextBlock
                    {
                        Text = character.Level.ToString(),
                        Font = charNameHeader.Font,
                        TextSize = charNameHeader.TextSize,
                        TextColor = charNameHeader.TextColor
                    };
                    levelfield.SetGridColumn(2);
                    levelfield.SetGridRow(i);
                    charGrid.Children.Add(levelfield);

                    var serverButton = new Button { MouseOverImage = buttonCreateCharacter.MouseOverImage, NotPressedImage = buttonCreateCharacter.NotPressedImage, PressedImage = buttonCreateCharacter.PressedImage };
                    var buttonLabel = new TextBlock { Text = "Play", Font = charNameHeader.Font, TextSize = 8, TextColor = charNameHeader.TextColor, VerticalAlignment = VerticalAlignment.Center, HorizontalAlignment = HorizontalAlignment.Center };
                    serverButton.Content = buttonLabel;
                    serverButton.SetGridColumn(3);
                    serverButton.SetGridRow(i++);
                    charGrid.Children.Add(serverButton);
                    serverButton.Click += (s, e) => {
                        ((Button)s).IsEnabled = false;
                        world.ResetAckForZone();
                        world.SendEnterWorld(new EnterWorld
                        {
                            Name = character.Name,
                            GoHome = false,
                            Tutorial = false
                        });
                    };
                }
            };

            world.ChatServerList += (sender, chatBytes) =>
            {
                world.ChatServers.Add(new ChatServer(chatBytes));
            };

            world.ZoneServer += (sender, server) =>
            {
                //WriteLine($"Zone server info: {server.Host}:{server.Port}");

                var zone = new ZoneStream(server.Host, server.Port, charname);
                SetupZone(zone);
            };
        }

        static void SetupZone(ZoneStream zone)
        {

        }

        public override void Update() {
        }
    }
}
