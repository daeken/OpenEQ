
namespace OpenEQ.Configuration
{
    using System;
    using System.IO;
    using Microsoft.Extensions.Configuration;

    public class OpenEQConfiguration
    {
        private static OpenEQConfiguration _instance;

        public static OpenEQConfiguration Instance => _instance ?? (_instance = new OpenEQConfiguration());

        private readonly IConfigurationRoot _configRoot;

        public string LoginServerAddress => _configRoot.GetSection("LoginServer")["Host"];

        public int LoginServerPort => Convert.ToInt32(_configRoot.GetSection("LoginServer")["Port"]);

        protected OpenEQConfiguration()
        {
            // So, LoginServer is overwritten because it exists in the xenko data files.  This needs to be
            // configurable so it's going to live here until I work out how else to do it because I know
            // nearly nothing about Xenko.
            var builder = new ConfigurationBuilder();
            builder.SetBasePath(Directory.GetCurrentDirectory());
            builder.AddJsonFile("appsettings.json", optional: false, reloadOnChange: false);
            _configRoot = builder.Build();
        }
    }
}