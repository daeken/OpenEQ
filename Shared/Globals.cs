﻿using System;
using System.Threading.Tasks;
using static System.Console;

namespace OpenEQ {
	public class AsyncHelper {
		public static void Run(Action func, bool longRunning = false) {
			var tlst = Environment.StackTrace;
			Task.Factory.StartNew(() => {
				try {
					func();
				} catch(Exception e) {
					WriteLine($"Async task threw exception ${e}");
					WriteLine(e.StackTrace);
					WriteLine("Outer stack trace:");
					WriteLine(tlst);
					System.Environment.Exit(0);
				}
			}, longRunning ? TaskCreationOptions.LongRunning : TaskCreationOptions.None);
		}
	}
	class Time {
		static long StartTicks = NowTicks;
		static long NowTicks => DateTime.Now.Ticks;
		public static float Now => (NowTicks - StartTicks) / 10000000f;
	}

	public enum ZoneNumber {
		Unknown = -1,
		qeynos = 1,
		qeynos2 = 2,
		qrg = 3,
		qeytoqrg = 4,
		highpass = 5,
		highkeep = 6,
		freportn = 8,
		freportw = 9,
		freporte = 10,
		runnyeye = 11,
		qey2hh1 = 12,
		northkarana = 13,
		southkarana = 14,
		eastkarana = 15,
		beholder = 16,
		blackburrow = 17,
		paw = 18,
		rivervale = 19,
		kithicor = 20,
		commons = 21,
		ecommons = 22,
		erudnint = 23,
		erudnext = 24,
		nektulos = 25,
		cshome = 26,
		lavastorm = 27,
		nektropos = 28,
		halas = 29,
		everfrost = 30,
		soldunga = 31,
		soldungb = 32,
		misty = 33,
		nro = 34,
		sro = 35,
		befallen = 36,
		oasis = 37,
		tox = 38,
		hole = 39,
		neriaka = 40,
		neriakb = 41,
		neriakc = 42,
		neriakd = 43,
		najena = 44,
		qcat = 45,
		innothule = 46,
		feerrott = 47,
		cazicthule = 48,
		oggok = 49,
		rathemtn = 50,
		lakerathe = 51,
		grobb = 52,
		aviak = 53,
		gfaydark = 54,
		akanon = 55,
		steamfont = 56,
		lfaydark = 57,
		crushbone = 58,
		mistmoore = 59,
		kaladima = 60,
		felwithea = 61,
		felwitheb = 62,
		unrest = 63,
		kedge = 64,
		guktop = 65,
		gukbottom = 66,
		kaladimb = 67,
		butcher = 68,
		oot = 69,
		cauldron = 70,
		airplane = 71,
		fearplane = 72,
		permafrost = 73,
		kerraridge = 74,
		paineel = 75,
		hateplane = 76,
		arena = 77,
		fieldofbone = 78,
		warslikswood = 79,
		soltemple = 80,
		droga = 81,
		cabwest = 82,
		swampofnohope = 83,
		firiona = 84,
		lakeofillomen = 85,
		dreadlands = 86,
		burningwood = 87,
		kaesora = 88,
		sebilis = 89,
		citymist = 90,
		skyfire = 91,
		frontiermtns = 92,
		overthere = 93,
		emeraldjungle = 94,
		trakanon = 95,
		timorous = 96,
		kurn = 97,
		erudsxing = 98,
		stonebrunt = 100,
		warrens = 101,
		karnor = 102,
		chardok = 103,
		dalnir = 104,
		charasis = 105,
		cabeast = 106,
		nurga = 107,
		veeshan = 108,
		veksar = 109,
		iceclad = 110,
		frozenshadow = 111,
		velketor = 112,
		kael = 113,
		skyshrine = 114,
		thurgadina = 115,
		eastwastes = 116,
		cobaltscar = 117,
		greatdivide = 118,
		wakening = 119,
		westwastes = 120,
		crystal = 121,
		necropolis = 123,
		templeveeshan = 124,
		sirens = 125,
		mischiefplane = 126,
		growthplane = 127,
		sleeper = 128,
		thurgadinb = 129,
		erudsxing2 = 130,
		shadowhaven = 150,
		bazaar = 151,
		nexus = 152,
		echo = 153,
		acrylia = 154,
		sharvahl = 155,
		paludal = 156,
		fungusgrove = 157,
		vexthal = 158,
		sseru = 159,
		katta = 160,
		netherbian = 161,
		ssratemple = 162,
		griegsend = 163,
		thedeep = 164,
		shadeweaver = 165,
		hollowshade = 166,
		grimling = 167,
		mseru = 168,
		letalis = 169,
		twilight = 170,
		thegrey = 171,
		tenebrous = 172,
		maiden = 173,
		dawnshroud = 174,
		scarlet = 175,
		umbral = 176,
		akheva = 179,
		arena2 = 180,
		jaggedpine = 181,
		nedaria = 182,
		tutorial = 183,
		load = 184,
		load2 = 185,
		hateplaneb = 186,
		shadowrest = 187,
		tutoriala = 188,
		tutorialb = 189,
		clz = 190,
		codecay = 200,
		pojustice = 201,
		poknowledge = 202,
		potranquility = 203,
		ponightmare = 204,
		podisease = 205,
		poinnovation = 206,
		potorment = 207,
		povalor = 208,
		bothunder = 209,
		postorms = 210,
		hohonora = 211,
		solrotower = 212,
		powar = 213,
		potactics = 214,
		poair = 215,
		powater = 216,
		pofire = 217,
		poeartha = 218,
		potimea = 219,
		hohonorb = 220,
		nightmareb = 221,
		poearthb = 222,
		potimeb = 223,
		gunthak = 224,
		dulak = 225,
		torgiran = 226,
		nadox = 227,
		hatesfury = 228,
		guka = 229,
		ruja = 230,
		taka = 231,
		mira = 232,
		mmca = 233,
		gukb = 234,
		rujb = 235,
		takb = 236,
		mirb = 237,
		mmcb = 238,
		gukc = 239,
		rujc = 240,
		takc = 241,
		mirc = 242,
		mmcc = 243,
		gukd = 244,
		rujd = 245,
		takd = 246,
		mird = 247,
		mmcd = 248,
		guke = 249,
		ruje = 250,
		take = 251,
		mire = 252,
		mmce = 253,
		gukf = 254,
		rujf = 255,
		takf = 256,
		mirf = 257,
		mmcf = 258,
		gukg = 259,
		rujg = 260,
		takg = 261,
		mirg = 262,
		mmcg = 263,
		gukh = 264,
		rujh = 265,
		takh = 266,
		mirh = 267,
		mmch = 268,
		ruji = 269,
		taki = 270,
		miri = 271,
		mmci = 272,
		rujj = 273,
		takj = 274,
		mirj = 275,
		mmcj = 276,
		chardokb = 277,
		soldungc = 278,
		abysmal = 279,
		natimbi = 280,
		qinimi = 281,
		riwwi = 282,
		barindu = 283,
		ferubi = 284,
		snpool = 285,
		snlair = 286,
		snplant = 287,
		sncrematory = 288,
		tipt = 289,
		vxed = 290,
		yxtta = 291,
		uqua = 292,
		kodtaz = 293,
		ikkinz = 294,
		qvic = 295,
		inktuta = 296,
		txevu = 297,
		tacvi = 298,
		qvicb = 299,
		wallofslaughter = 300,
		bloodfields = 301,
		draniksscar = 302,
		causeway = 303,
		chambersa = 304,
		chambersb = 305,
		chambersc = 306,
		chambersd = 307,
		chamberse = 308,
		chambersf = 309,
		provinggrounds = 316,
		anguish = 317,
		dranikhollowsa = 318,
		dranikhollowsb = 319,
		dranikhollowsc = 320,
		dranikcatacombsa = 328,
		dranikcatacombsb = 329,
		dranikcatacombsc = 330,
		draniksewersa = 331,
		draniksewersb = 332,
		draniksewersc = 333,
		riftseekers = 334,
		harbingers = 335,
		dranik = 336,
		broodlands = 337,
		stillmoona = 338,
		stillmoonb = 339,
		thundercrest = 340,
		delvea = 341,
		delveb = 342,
		thenest = 343,
		guildlobby = 344,
		guildhall = 345,
		barter = 346,
		illsalin = 347,
		illsalina = 348,
		illsalinb = 349,
		illsalinc = 350,
		dreadspire = 351,
		drachnidhive = 354,
		drachnidhivea = 355,
		drachnidhiveb = 356,
		drachnidhivec = 357,
		westkorlach = 358,
		westkorlacha = 359,
		westkorlachb = 360,
		westkorlachc = 361,
		eastkorlach = 362,
		eastkorlacha = 363,
		shadowspine = 364,
		corathus = 365,
		corathusa = 366,
		corathusb = 367,
		nektulosa = 368,
		arcstone = 369,
		relic = 370,
		skylance = 371,
		devastation = 372,
		devastationa = 373,
		rage = 374,
		ragea = 375,
		takishruins = 376,
		takishruinsa = 377,
		elddar = 378,
		elddara = 379,
		theater = 380,
		theatera = 381,
		freeporteast = 382,
		freeportwest = 383,
		freeportsewers = 384,
		freeportacademy = 385,
		freeporttemple = 386,
		freeportmilitia = 387,
		freeportarena = 388,
		freeportcityhall = 389,
		freeporttheater = 390,
		freeporthall = 391,
		northro = 392,
		southro = 393,
		crescent = 394,
		moors = 395,
		stonehive = 396,
		mesa = 397,
		roost = 398,
		steppes = 399,
		icefall = 400,
		valdeholm = 401,
		frostcrypt = 402,
		sunderock = 403,
		vergalid = 404,
		direwind = 405,
		ashengate = 406,
		highpasshold = 407,
		commonlands = 408,
		oceanoftears = 409,
		kithforest = 410,
		befallenb = 411,
		highpasskeep = 412,
		innothuleb = 413,
		toxxulia = 414,
		mistythicket = 415,
		kattacastrum = 416,
		thalassius = 417,
		atiiki = 418,
		zhisza = 419,
		silyssar = 420,
		solteris = 421,
		barren = 422,
		buriedsea = 423,
		jardelshook = 424,
		monkeyrock = 425,
		suncrest = 426,
		deadbone = 427,
		blacksail = 428,
		maidensgrave = 429,
		redfeather = 430,
		shipmvp = 431,
		shipmvu = 432,
		shippvu = 433,
		shipuvu = 434,
		shipmvm = 435,
		mechanotus = 436,
		mansion = 437,
		steamfactory = 438,
		shipworkshop = 439,
		gyrospireb = 440,
		gyrospirez = 441,
		dragonscale = 442,
		lopingplains = 443,
		hillsofshade = 444,
		bloodmoon = 445,
		crystallos = 446,
		guardian = 447,
		steamfontmts = 448,
		cryptofshade = 449,
		dragonscaleb = 451,
		oldfieldofbone = 452,
		oldkaesoraa = 453,
		oldkaesorab = 454,
		oldkurn = 455,
		oldkithicor = 456,
		oldcommons = 457,
		oldhighpass = 458,
		thevoida = 459,
		thevoidb = 460,
		thevoidc = 461,
		thevoidd = 462,
		thevoide = 463,
		thevoidf = 464,
		thevoidg = 465,
		oceangreenhills = 466,
		oceangreenvillage = 467,
		oldblackburrow = 468,
		bertoxtemple = 469,
		discord = 470,
		discordtower = 471,
		oldbloodfield = 472,
		precipiceofwar = 473,
		olddranik = 474,
		toskirakk = 475,
		korascian = 476,
		rathechamber = 477,
		brellsrest = 480,
		fungalforest = 481,
		underquarry = 482,
		coolingchamber = 483,
		shiningcity = 484,
		arthicrex = 485,
		foundation = 486,
		lichencreep = 487,
		pellucid = 488,
		stonesnake = 489,
		brellstemple = 490,
		convorteum = 491,
		brellsarena = 492,
		weddingchapel = 493,
		weddingchapeldark = 494,
		dragoncrypt = 495,
		feerrott2 = 700,
		thulehouse1 = 701,
		thulehouse2 = 702,
		housegarden = 703,
		thulelibrary = 704,
		well = 705,
		fallen = 706,
		morellcastle = 707,
		somnium = 708,
		alkabormare = 709,
		miragulmare = 710,
		thuledream = 711,
		neighborhood = 712,
		argath = 724,
		arelis = 725,
		sarithcity = 726,
		rubak = 727,
		beastdomain = 728,
		resplendent = 729,
		pillarsalra = 730,
		windsong = 731,
		cityofbronze = 732,
		sepulcher = 733,
		eastsepulcher = 734,
		westsepulcher = 735,
		shardslanding = 752,
		xorbb = 753,
		kaelshard = 754,
		eastwastesshard = 755,
		crystalshard = 756,
		breedinggrounds = 757,
		eviltree = 758,
		grelleth = 759,
		chapterhouse = 760,
		arttest = 996,
		fhalls = 998,
		apprentice = 999
	}
}
