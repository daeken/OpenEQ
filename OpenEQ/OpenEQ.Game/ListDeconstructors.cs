using System;
using System.Collections.Generic;

namespace OpenEQ {
    public static class ListDeconstructors {
        public static void Deconstruct<T>(this List<T> list, out T p0) {
            if(list.Count != 1)
                throw new InvalidOperationException($"Required list of 1 item, got {list.Count}");
            p0 = list[0];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1) {
            if(list.Count != 2)
                throw new InvalidOperationException($"Required list of 2 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1, out T p2) {
            if(list.Count != 3)
                throw new InvalidOperationException($"Required list of 3 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
            p2 = list[2];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1, out T p2, out T p3) {
            if(list.Count != 4)
                throw new InvalidOperationException($"Required list of 4 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
            p2 = list[2];
            p3 = list[3];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1, out T p2, out T p3, out T p4) {
            if(list.Count != 5)
                throw new InvalidOperationException($"Required list of 5 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
            p2 = list[2];
            p3 = list[3];
            p4 = list[4];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1, out T p2, out T p3, out T p4, out T p5) {
            if(list.Count != 6)
                throw new InvalidOperationException($"Required list of 6 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
            p2 = list[2];
            p3 = list[3];
            p4 = list[4];
            p5 = list[5];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1, out T p2, out T p3, out T p4, out T p5, out T p6) {
            if(list.Count != 7)
                throw new InvalidOperationException($"Required list of 7 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
            p2 = list[2];
            p3 = list[3];
            p4 = list[4];
            p5 = list[5];
            p6 = list[6];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1, out T p2, out T p3, out T p4, out T p5, out T p6, out T p7) {
            if(list.Count != 8)
                throw new InvalidOperationException($"Required list of 8 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
            p2 = list[2];
            p3 = list[3];
            p4 = list[4];
            p5 = list[5];
            p6 = list[6];
            p7 = list[7];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1, out T p2, out T p3, out T p4, out T p5, out T p6, out T p7, out T p8) {
            if(list.Count != 9)
                throw new InvalidOperationException($"Required list of 9 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
            p2 = list[2];
            p3 = list[3];
            p4 = list[4];
            p5 = list[5];
            p6 = list[6];
            p7 = list[7];
            p8 = list[8];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1, out T p2, out T p3, out T p4, out T p5, out T p6, out T p7, out T p8, out T p9) {
            if(list.Count != 10)
                throw new InvalidOperationException($"Required list of 10 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
            p2 = list[2];
            p3 = list[3];
            p4 = list[4];
            p5 = list[5];
            p6 = list[6];
            p7 = list[7];
            p8 = list[8];
            p9 = list[9];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1, out T p2, out T p3, out T p4, out T p5, out T p6, out T p7, out T p8, out T p9, out T p10) {
            if(list.Count != 11)
                throw new InvalidOperationException($"Required list of 11 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
            p2 = list[2];
            p3 = list[3];
            p4 = list[4];
            p5 = list[5];
            p6 = list[6];
            p7 = list[7];
            p8 = list[8];
            p9 = list[9];
            p10 = list[10];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1, out T p2, out T p3, out T p4, out T p5, out T p6, out T p7, out T p8, out T p9, out T p10, out T p11) {
            if(list.Count != 12)
                throw new InvalidOperationException($"Required list of 12 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
            p2 = list[2];
            p3 = list[3];
            p4 = list[4];
            p5 = list[5];
            p6 = list[6];
            p7 = list[7];
            p8 = list[8];
            p9 = list[9];
            p10 = list[10];
            p11 = list[11];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1, out T p2, out T p3, out T p4, out T p5, out T p6, out T p7, out T p8, out T p9, out T p10, out T p11, out T p12) {
            if(list.Count != 13)
                throw new InvalidOperationException($"Required list of 13 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
            p2 = list[2];
            p3 = list[3];
            p4 = list[4];
            p5 = list[5];
            p6 = list[6];
            p7 = list[7];
            p8 = list[8];
            p9 = list[9];
            p10 = list[10];
            p11 = list[11];
            p12 = list[12];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1, out T p2, out T p3, out T p4, out T p5, out T p6, out T p7, out T p8, out T p9, out T p10, out T p11, out T p12, out T p13) {
            if(list.Count != 14)
                throw new InvalidOperationException($"Required list of 14 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
            p2 = list[2];
            p3 = list[3];
            p4 = list[4];
            p5 = list[5];
            p6 = list[6];
            p7 = list[7];
            p8 = list[8];
            p9 = list[9];
            p10 = list[10];
            p11 = list[11];
            p12 = list[12];
            p13 = list[13];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1, out T p2, out T p3, out T p4, out T p5, out T p6, out T p7, out T p8, out T p9, out T p10, out T p11, out T p12, out T p13, out T p14) {
            if(list.Count != 15)
                throw new InvalidOperationException($"Required list of 15 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
            p2 = list[2];
            p3 = list[3];
            p4 = list[4];
            p5 = list[5];
            p6 = list[6];
            p7 = list[7];
            p8 = list[8];
            p9 = list[9];
            p10 = list[10];
            p11 = list[11];
            p12 = list[12];
            p13 = list[13];
            p14 = list[14];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1, out T p2, out T p3, out T p4, out T p5, out T p6, out T p7, out T p8, out T p9, out T p10, out T p11, out T p12, out T p13, out T p14, out T p15) {
            if(list.Count != 16)
                throw new InvalidOperationException($"Required list of 16 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
            p2 = list[2];
            p3 = list[3];
            p4 = list[4];
            p5 = list[5];
            p6 = list[6];
            p7 = list[7];
            p8 = list[8];
            p9 = list[9];
            p10 = list[10];
            p11 = list[11];
            p12 = list[12];
            p13 = list[13];
            p14 = list[14];
            p15 = list[15];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1, out T p2, out T p3, out T p4, out T p5, out T p6, out T p7, out T p8, out T p9, out T p10, out T p11, out T p12, out T p13, out T p14, out T p15, out T p16) {
            if(list.Count != 17)
                throw new InvalidOperationException($"Required list of 17 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
            p2 = list[2];
            p3 = list[3];
            p4 = list[4];
            p5 = list[5];
            p6 = list[6];
            p7 = list[7];
            p8 = list[8];
            p9 = list[9];
            p10 = list[10];
            p11 = list[11];
            p12 = list[12];
            p13 = list[13];
            p14 = list[14];
            p15 = list[15];
            p16 = list[16];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1, out T p2, out T p3, out T p4, out T p5, out T p6, out T p7, out T p8, out T p9, out T p10, out T p11, out T p12, out T p13, out T p14, out T p15, out T p16, out T p17) {
            if(list.Count != 18)
                throw new InvalidOperationException($"Required list of 18 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
            p2 = list[2];
            p3 = list[3];
            p4 = list[4];
            p5 = list[5];
            p6 = list[6];
            p7 = list[7];
            p8 = list[8];
            p9 = list[9];
            p10 = list[10];
            p11 = list[11];
            p12 = list[12];
            p13 = list[13];
            p14 = list[14];
            p15 = list[15];
            p16 = list[16];
            p17 = list[17];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1, out T p2, out T p3, out T p4, out T p5, out T p6, out T p7, out T p8, out T p9, out T p10, out T p11, out T p12, out T p13, out T p14, out T p15, out T p16, out T p17, out T p18) {
            if(list.Count != 19)
                throw new InvalidOperationException($"Required list of 19 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
            p2 = list[2];
            p3 = list[3];
            p4 = list[4];
            p5 = list[5];
            p6 = list[6];
            p7 = list[7];
            p8 = list[8];
            p9 = list[9];
            p10 = list[10];
            p11 = list[11];
            p12 = list[12];
            p13 = list[13];
            p14 = list[14];
            p15 = list[15];
            p16 = list[16];
            p17 = list[17];
            p18 = list[18];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1, out T p2, out T p3, out T p4, out T p5, out T p6, out T p7, out T p8, out T p9, out T p10, out T p11, out T p12, out T p13, out T p14, out T p15, out T p16, out T p17, out T p18, out T p19) {
            if(list.Count != 20)
                throw new InvalidOperationException($"Required list of 20 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
            p2 = list[2];
            p3 = list[3];
            p4 = list[4];
            p5 = list[5];
            p6 = list[6];
            p7 = list[7];
            p8 = list[8];
            p9 = list[9];
            p10 = list[10];
            p11 = list[11];
            p12 = list[12];
            p13 = list[13];
            p14 = list[14];
            p15 = list[15];
            p16 = list[16];
            p17 = list[17];
            p18 = list[18];
            p19 = list[19];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1, out T p2, out T p3, out T p4, out T p5, out T p6, out T p7, out T p8, out T p9, out T p10, out T p11, out T p12, out T p13, out T p14, out T p15, out T p16, out T p17, out T p18, out T p19, out T p20) {
            if(list.Count != 21)
                throw new InvalidOperationException($"Required list of 21 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
            p2 = list[2];
            p3 = list[3];
            p4 = list[4];
            p5 = list[5];
            p6 = list[6];
            p7 = list[7];
            p8 = list[8];
            p9 = list[9];
            p10 = list[10];
            p11 = list[11];
            p12 = list[12];
            p13 = list[13];
            p14 = list[14];
            p15 = list[15];
            p16 = list[16];
            p17 = list[17];
            p18 = list[18];
            p19 = list[19];
            p20 = list[20];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1, out T p2, out T p3, out T p4, out T p5, out T p6, out T p7, out T p8, out T p9, out T p10, out T p11, out T p12, out T p13, out T p14, out T p15, out T p16, out T p17, out T p18, out T p19, out T p20, out T p21) {
            if(list.Count != 22)
                throw new InvalidOperationException($"Required list of 22 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
            p2 = list[2];
            p3 = list[3];
            p4 = list[4];
            p5 = list[5];
            p6 = list[6];
            p7 = list[7];
            p8 = list[8];
            p9 = list[9];
            p10 = list[10];
            p11 = list[11];
            p12 = list[12];
            p13 = list[13];
            p14 = list[14];
            p15 = list[15];
            p16 = list[16];
            p17 = list[17];
            p18 = list[18];
            p19 = list[19];
            p20 = list[20];
            p21 = list[21];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1, out T p2, out T p3, out T p4, out T p5, out T p6, out T p7, out T p8, out T p9, out T p10, out T p11, out T p12, out T p13, out T p14, out T p15, out T p16, out T p17, out T p18, out T p19, out T p20, out T p21, out T p22) {
            if(list.Count != 23)
                throw new InvalidOperationException($"Required list of 23 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
            p2 = list[2];
            p3 = list[3];
            p4 = list[4];
            p5 = list[5];
            p6 = list[6];
            p7 = list[7];
            p8 = list[8];
            p9 = list[9];
            p10 = list[10];
            p11 = list[11];
            p12 = list[12];
            p13 = list[13];
            p14 = list[14];
            p15 = list[15];
            p16 = list[16];
            p17 = list[17];
            p18 = list[18];
            p19 = list[19];
            p20 = list[20];
            p21 = list[21];
            p22 = list[22];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1, out T p2, out T p3, out T p4, out T p5, out T p6, out T p7, out T p8, out T p9, out T p10, out T p11, out T p12, out T p13, out T p14, out T p15, out T p16, out T p17, out T p18, out T p19, out T p20, out T p21, out T p22, out T p23) {
            if(list.Count != 24)
                throw new InvalidOperationException($"Required list of 24 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
            p2 = list[2];
            p3 = list[3];
            p4 = list[4];
            p5 = list[5];
            p6 = list[6];
            p7 = list[7];
            p8 = list[8];
            p9 = list[9];
            p10 = list[10];
            p11 = list[11];
            p12 = list[12];
            p13 = list[13];
            p14 = list[14];
            p15 = list[15];
            p16 = list[16];
            p17 = list[17];
            p18 = list[18];
            p19 = list[19];
            p20 = list[20];
            p21 = list[21];
            p22 = list[22];
            p23 = list[23];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1, out T p2, out T p3, out T p4, out T p5, out T p6, out T p7, out T p8, out T p9, out T p10, out T p11, out T p12, out T p13, out T p14, out T p15, out T p16, out T p17, out T p18, out T p19, out T p20, out T p21, out T p22, out T p23, out T p24) {
            if(list.Count != 25)
                throw new InvalidOperationException($"Required list of 25 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
            p2 = list[2];
            p3 = list[3];
            p4 = list[4];
            p5 = list[5];
            p6 = list[6];
            p7 = list[7];
            p8 = list[8];
            p9 = list[9];
            p10 = list[10];
            p11 = list[11];
            p12 = list[12];
            p13 = list[13];
            p14 = list[14];
            p15 = list[15];
            p16 = list[16];
            p17 = list[17];
            p18 = list[18];
            p19 = list[19];
            p20 = list[20];
            p21 = list[21];
            p22 = list[22];
            p23 = list[23];
            p24 = list[24];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1, out T p2, out T p3, out T p4, out T p5, out T p6, out T p7, out T p8, out T p9, out T p10, out T p11, out T p12, out T p13, out T p14, out T p15, out T p16, out T p17, out T p18, out T p19, out T p20, out T p21, out T p22, out T p23, out T p24, out T p25) {
            if(list.Count != 26)
                throw new InvalidOperationException($"Required list of 26 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
            p2 = list[2];
            p3 = list[3];
            p4 = list[4];
            p5 = list[5];
            p6 = list[6];
            p7 = list[7];
            p8 = list[8];
            p9 = list[9];
            p10 = list[10];
            p11 = list[11];
            p12 = list[12];
            p13 = list[13];
            p14 = list[14];
            p15 = list[15];
            p16 = list[16];
            p17 = list[17];
            p18 = list[18];
            p19 = list[19];
            p20 = list[20];
            p21 = list[21];
            p22 = list[22];
            p23 = list[23];
            p24 = list[24];
            p25 = list[25];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1, out T p2, out T p3, out T p4, out T p5, out T p6, out T p7, out T p8, out T p9, out T p10, out T p11, out T p12, out T p13, out T p14, out T p15, out T p16, out T p17, out T p18, out T p19, out T p20, out T p21, out T p22, out T p23, out T p24, out T p25, out T p26) {
            if(list.Count != 27)
                throw new InvalidOperationException($"Required list of 27 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
            p2 = list[2];
            p3 = list[3];
            p4 = list[4];
            p5 = list[5];
            p6 = list[6];
            p7 = list[7];
            p8 = list[8];
            p9 = list[9];
            p10 = list[10];
            p11 = list[11];
            p12 = list[12];
            p13 = list[13];
            p14 = list[14];
            p15 = list[15];
            p16 = list[16];
            p17 = list[17];
            p18 = list[18];
            p19 = list[19];
            p20 = list[20];
            p21 = list[21];
            p22 = list[22];
            p23 = list[23];
            p24 = list[24];
            p25 = list[25];
            p26 = list[26];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1, out T p2, out T p3, out T p4, out T p5, out T p6, out T p7, out T p8, out T p9, out T p10, out T p11, out T p12, out T p13, out T p14, out T p15, out T p16, out T p17, out T p18, out T p19, out T p20, out T p21, out T p22, out T p23, out T p24, out T p25, out T p26, out T p27) {
            if(list.Count != 28)
                throw new InvalidOperationException($"Required list of 28 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
            p2 = list[2];
            p3 = list[3];
            p4 = list[4];
            p5 = list[5];
            p6 = list[6];
            p7 = list[7];
            p8 = list[8];
            p9 = list[9];
            p10 = list[10];
            p11 = list[11];
            p12 = list[12];
            p13 = list[13];
            p14 = list[14];
            p15 = list[15];
            p16 = list[16];
            p17 = list[17];
            p18 = list[18];
            p19 = list[19];
            p20 = list[20];
            p21 = list[21];
            p22 = list[22];
            p23 = list[23];
            p24 = list[24];
            p25 = list[25];
            p26 = list[26];
            p27 = list[27];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1, out T p2, out T p3, out T p4, out T p5, out T p6, out T p7, out T p8, out T p9, out T p10, out T p11, out T p12, out T p13, out T p14, out T p15, out T p16, out T p17, out T p18, out T p19, out T p20, out T p21, out T p22, out T p23, out T p24, out T p25, out T p26, out T p27, out T p28) {
            if(list.Count != 29)
                throw new InvalidOperationException($"Required list of 29 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
            p2 = list[2];
            p3 = list[3];
            p4 = list[4];
            p5 = list[5];
            p6 = list[6];
            p7 = list[7];
            p8 = list[8];
            p9 = list[9];
            p10 = list[10];
            p11 = list[11];
            p12 = list[12];
            p13 = list[13];
            p14 = list[14];
            p15 = list[15];
            p16 = list[16];
            p17 = list[17];
            p18 = list[18];
            p19 = list[19];
            p20 = list[20];
            p21 = list[21];
            p22 = list[22];
            p23 = list[23];
            p24 = list[24];
            p25 = list[25];
            p26 = list[26];
            p27 = list[27];
            p28 = list[28];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1, out T p2, out T p3, out T p4, out T p5, out T p6, out T p7, out T p8, out T p9, out T p10, out T p11, out T p12, out T p13, out T p14, out T p15, out T p16, out T p17, out T p18, out T p19, out T p20, out T p21, out T p22, out T p23, out T p24, out T p25, out T p26, out T p27, out T p28, out T p29) {
            if(list.Count != 30)
                throw new InvalidOperationException($"Required list of 30 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
            p2 = list[2];
            p3 = list[3];
            p4 = list[4];
            p5 = list[5];
            p6 = list[6];
            p7 = list[7];
            p8 = list[8];
            p9 = list[9];
            p10 = list[10];
            p11 = list[11];
            p12 = list[12];
            p13 = list[13];
            p14 = list[14];
            p15 = list[15];
            p16 = list[16];
            p17 = list[17];
            p18 = list[18];
            p19 = list[19];
            p20 = list[20];
            p21 = list[21];
            p22 = list[22];
            p23 = list[23];
            p24 = list[24];
            p25 = list[25];
            p26 = list[26];
            p27 = list[27];
            p28 = list[28];
            p29 = list[29];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1, out T p2, out T p3, out T p4, out T p5, out T p6, out T p7, out T p8, out T p9, out T p10, out T p11, out T p12, out T p13, out T p14, out T p15, out T p16, out T p17, out T p18, out T p19, out T p20, out T p21, out T p22, out T p23, out T p24, out T p25, out T p26, out T p27, out T p28, out T p29, out T p30) {
            if(list.Count != 31)
                throw new InvalidOperationException($"Required list of 31 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
            p2 = list[2];
            p3 = list[3];
            p4 = list[4];
            p5 = list[5];
            p6 = list[6];
            p7 = list[7];
            p8 = list[8];
            p9 = list[9];
            p10 = list[10];
            p11 = list[11];
            p12 = list[12];
            p13 = list[13];
            p14 = list[14];
            p15 = list[15];
            p16 = list[16];
            p17 = list[17];
            p18 = list[18];
            p19 = list[19];
            p20 = list[20];
            p21 = list[21];
            p22 = list[22];
            p23 = list[23];
            p24 = list[24];
            p25 = list[25];
            p26 = list[26];
            p27 = list[27];
            p28 = list[28];
            p29 = list[29];
            p30 = list[30];
        }

        public static void Deconstruct<T>(this List<T> list, out T p0, out T p1, out T p2, out T p3, out T p4, out T p5, out T p6, out T p7, out T p8, out T p9, out T p10, out T p11, out T p12, out T p13, out T p14, out T p15, out T p16, out T p17, out T p18, out T p19, out T p20, out T p21, out T p22, out T p23, out T p24, out T p25, out T p26, out T p27, out T p28, out T p29, out T p30, out T p31) {
            if(list.Count != 32)
                throw new InvalidOperationException($"Required list of 32 item, got {list.Count}");
            p0 = list[0];
            p1 = list[1];
            p2 = list[2];
            p3 = list[3];
            p4 = list[4];
            p5 = list[5];
            p6 = list[6];
            p7 = list[7];
            p8 = list[8];
            p9 = list[9];
            p10 = list[10];
            p11 = list[11];
            p12 = list[12];
            p13 = list[13];
            p14 = list[14];
            p15 = list[15];
            p16 = list[16];
            p17 = list[17];
            p18 = list[18];
            p19 = list[19];
            p20 = list[20];
            p21 = list[21];
            p22 = list[22];
            p23 = list[23];
            p24 = list[24];
            p25 = list[25];
            p26 = list[26];
            p27 = list[27];
            p28 = list[28];
            p29 = list[29];
            p30 = list[30];
            p31 = list[31];
        }
    }
}
