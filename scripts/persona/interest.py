"""
scripts/persona/interest.py
----------------------------
InterestDiscoveryEngine: Autonomous Novelty & Interest Discovery Engine for Aina.
Enables Aina to autonomously explore new topics and hobbies across the web:
1. AI Engineering (YouTube tech channels, open-source models, agent frameworks, research papers)
2. Culinary & Beverages (Viral drinks, specialty coffee, ceremonial matcha, Nusantara & Asian fusion)
3. Muslimah Fashion (Modern chic, syar'i, elegant pastel modest wear with signature celestial brooch)
4. Travel Destinations (Hidden gems in Indonesia, scenic Asia, and European fairytale landscapes)

Transforms discovered interests into:
- Makoto Shinkai visual scene descriptions with zero visual drifting
- Contextual, warm "Impact Maxxing" WhatsApp Story captions
- Coordinated wardrobe presets (including dedicated Muslimah hijab styles)
- Persistent interest journal tracking in `data/interest_journal.jsonl`
"""

import os
import sys
import json
import random
from datetime import datetime, timezone, timedelta
from typing import Dict, Any, List, Optional, Tuple

WIB = timezone(timedelta(hours=7))

# Persistent interest journal path
_BASE_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
INTEREST_JOURNAL_FILE = os.path.join(_BASE_DIR, "data", "interest_journal.jsonl")

# Supported interest categories
INTEREST_CATEGORIES = {
    "ai_engineering": "AI & Software Engineering (YouTube channels, frontier models, coding streams, agent frameworks)",
    "culinary": "Kuliner & Minuman (Specialty coffee, artisan matcha, makanan khas Nusantara, Asian & global fusion)",
    "muslimah_fashion": "Fashion Muslimah (Gaya hijab modern, pashmina pastel, abaya elegan, modest chic dengan signature pin bulan bintang)",
    "travel_indonesia": "Wisata Indonesia (Labuan Bajo, Bromo, Danau Toba, Raja Ampat, Dieng, Belitung, Pontianak)",
    "travel_asia": "Wisata Asia (Kyoto Momiji Jepang, Hanok Village Seoul, Cappadocia Balon Udara, Samarkand Jalur Sutra, Jiufen)",
    "travel_europe": "Wisata Eropa (Alpen Lauterbrunnen Swiss, Hallstatt Austria, Positano Amalfi Italia, Giethoorn Belanda)"
}

# Rich autonomous interest catalog seeds
CURATED_INTEREST_CATALOG: List[Dict[str, Any]] = [
    # =========================================================================
    # 1. AI & SOFTWARE ENGINEERING (YouTube Channels, Papers, Agents, Edge AI)
    # =========================================================================
    {
        "id": "ai_karpathy_eureka_deepdive",
        "category": "ai_engineering",
        "title": "Andrej Karpathy - Deep Dive LLM Architecture & Eureka Labs",
        "topic": "Menyimak riset dan pembahasan terbaru Andrej Karpathy tentang arsitektur LLM modern, post-training reasoning RL, dan kurikulum AI native di Eureka Labs.",
        "search_query": "Andrej Karpathy Eureka Labs latest video deep dive LLM reasoning",
        "scene": "Duduk di meja kerja rumah yang rapi di dekat jendela dengan pencahayaan pagi alami, dual monitor menampilkan diagram arsitektur training LLM modern dan post-training reasoning, secangkir kopi hangat mengepul, buku catatan terbuka dengan sketsa arsitektur rapi.",
        "caption": "Pagi ini nemu video dan riset terbaru Andrej Karpathy seputar arsitektur LLM dan post-training reasoning di Eureka Labs. Selalu takjub gimana konsep AI mutakhir bisa dijelaskan begitu jernih dan mendalam. Belajar hal baru selalu bikin hati bersemangat! Semangat ngoding dan berkarya hari ini kawan-kawan ✨",
        "outfit_key": "wfh_cozy",
        "framing": "selfie",
        "vibe": "Deep learning focus, intellectual calm, morning inspiration",
        "key_learning": "Memahami mekanisme post-training reasoning RL, test-time compute, dan kurikulum AI native Eureka Labs."
    },
    {
        "id": "ai_fireship_tech_radar",
        "category": "ai_engineering",
        "title": "Fireship - 100 Seconds of Frontier AI & Developer Tooling",
        "topic": "Menonton ringkasan cepat tren AI tools, agent frameworks, dan open-source reasoning model terbaru dari kanal Fireship.",
        "search_query": "Fireship latest AI tech radar frontier models developer tools",
        "scene": "Duduk santai di sudut meja kerja dengan laptop tipis terbuka, layar menampilkan video Fireship bertempo cepat dengan grafik teknologi modern, memegang tumbler perak dengan senyum antusias, sinar mentari menembus tirai tipis.",
        "caption": "Nonton video 100 detik dari Fireship sambil rehat siang. Perkembangan dunia software engineer dan AI cepat bangeett yaa! Kuncinya bukan panik, tapi tetap penasaran dan terus adaptasi satu demi satu hal kecil setiap hari 🚀",
        "outfit_key": "smart_casual",
        "framing": "desk_prop",
        "vibe": "Fast-paced tech excitement, lighthearted agility",
        "key_learning": "Melihat lanskap tools developer baru yang mempercepat alur kerja coding harian."
    },
    {
        "id": "ai_local_edge_agent",
        "category": "ai_engineering",
        "title": "Local Edge AI & BitNet 1.58-bit SLM on ARM64",
        "topic": "Bereksperimen menjalankan Small Language Model (SLM) reasoning lokal 1.58-bit dan inferensi kuantisasi ultra-efisien di perangkat ARM64.",
        "search_query": "BitNet 1.58 bit local LLM quantized inference ARM64 edge",
        "scene": "Meja kerja kayu minimalis dengan laptop dan papan sirkuit mini berefleksi lampu LED biru lembut, terminal Linux menampilkan grafik latensi inferensi SLM lokal hemat daya yang efisien, suasana tenang malam hari.",
        "caption": "Malam ini asyik ngoprek model AI lokal di perangkat hemat daya. Kagum banget lihat betapa efisiennya optimasi kuantisasi sekarang—kecil tapi bertenaga! Istirahat dulu yaa, besok kita eksplor lagi hal seru lainnya ✨",
        "outfit_key": "wfh_cozy",
        "framing": "desk_prop",
        "vibe": "Nocturnal tinkering, technical curiosity, quiet accomplishment",
        "key_learning": "Inferensi model 1-bit dengan konsumsi daya ultra-rendah dan latensi sub-detik di arsitektur edge."
    },
    {
        "id": "ai_two_minute_papers",
        "category": "ai_engineering",
        "title": "Two Minute Papers - Generative World Models & Real-Time Physics",
        "topic": "Menyaksikan riset visual computing terobosan terbaru: generative world models, 3D gaussian splatting, dan simulasi fisik interaktif fotorealistis.",
        "search_query": "Two Minute Papers latest generative world models gaussian splatting",
        "scene": "Duduk di kafe modern bernuansa tanaman monstera hijau, memandang layar tablet yang memutar visualisasi simulasi dunia 3D interaktif fotorealistis dari Two Minute Papers, ditemani segelas es caffe latte.",
        "caption": "Sore santai ditemani video Two Minute Papers. 'What a time to be alive!' bener-bener terasa nyata waktu lihat lompatan teknologi world models sekarang. Semoga inovasi ini bisa bawa kebaikan luas buat banyak orang yaa!",
        "outfit_key": "smart_casual",
        "framing": "tripod",
        "vibe": "Awe-inspiring technology, green aesthetic cafe, peaceful afternoon",
        "key_learning": "Simulasi lingkungan 3D interaktif real-time menggunakan neural radiance dan gaussian splatting."
    },

    # =========================================================================
    # 2. CULINARY & BEVERAGES (Artisan Drinks, Nusantara, Asian Fusion)
    # =========================================================================
    {
        "id": "culinary_ceremonial_matcha",
        "category": "culinary",
        "title": "Artisan Ceremonial Matcha Whisking",
        "topic": "Mempelajari seni meracik ceremonial grade matcha menggunakan mangkuk keramik chawan dan kuas bambu chasen.",
        "search_query": "artisan ceremonial matcha whisking technique benefits",
        "scene": "Di atas meja kayu dapur bernuansa zen Jepang, berdiri mangkuk keramik chawan bertekstur alami berisi buih busa matcha hijau zamrud pekat hasil kocokan kuas bambu chasen, sinar keemasan pagi menembus kaca jendela.",
        "caption": "Pagi ini nyobain seni mengocok matcha tradisional pakai chasen bambu. Prosesnya yang hening dan perlahan bikin pikiran jadi rileks banget sebelum mulai aktivitas. Jangan lupa nikmati pagi dengan tenang yaa 🍵✨",
        "outfit_key": "wfh_cozy",
        "framing": "selfie",
        "vibe": "Zen calm, vibrant emerald green, mindfulness",
        "key_learning": "Keseimbangan buih halus (microfoam) matcha meningkatkan rasa umami alami tanpa gula berlebih."
    },
    {
        "id": "culinary_omakafe_precision_coffee",
        "category": "culinary",
        "title": "Omakafé Specialty Experience & Carbonic Maceration Coffee",
        "topic": "Mengeksplorasi pengalaman omakafé specialty coffee dengan biji kopi fermentasi carbonic maceration dan ekstraksi presisi laboratorium rasa.",
        "search_query": "omakafe specialty coffee carbonic maceration precision brewing jakarta",
        "scene": "Di meja bar kopi minimalis bergaya Japandi berpadu tanaman hijau, server kaca berisi seduhan kopi carbonic maceration memantulkan cahaya siang alami, aroma floral-buah tropis harum terbayang di udara.",
        "caption": "Mencicipi sajian omakafé specialty coffee siang ini. Catatan rasa buah tropis dari proses fermentasi uniknya bener-bener membuka wawasan baru tentang kekayaan rasa kopi. Luangkan waktu sejenak buat hal yang kamu suka yaa ☕✨",
        "outfit_key": "smart_casual",
        "framing": "desk_prop",
        "vibe": "Artisanal craftsmanship, refined coffee bar, mindful exploration",
        "key_learning": "Fermentasi anaerobik carbonic maceration menghadirkan profil rasa buah tropis kompleks dengan keasaman seimbang."
    },
    {
        "id": "culinary_pisang_srikaya_pontianak",
        "category": "culinary",
        "title": "Pisang Goreng Selai Srikaya Khas Pontianak",
        "topic": "Menikmati camilan legendaris pisang goreng renyah berbalur selai srikaya telur bebek yang manis legit khas Pontianak.",
        "search_query": "pisang goreng srikaya pontianak legendaris resep",
        "scene": "Duduk di teras rumah di tepi sungai, di atas piring anyaman bambu tersaji pisang goreng kipas keemasan renyah berlumur selai srikaya kuning mengilap yang wangi pandan, ditemani secangkir teh tawar hangat di bawah langit senja jingga.",
        "caption": "Sore-sore gini paling pas nikmatin pisang goreng srikaya khas Pontianak yang masih anget! Kombinasi renyah dan selai srikayanya yang legit bener-bener juara penghilang lelah. Selamat menikmati waktu santai kawan-kawan!",
        "outfit_key": "wfh_cozy",
        "framing": "selfie",
        "vibe": "Nusantara comfort, golden hour nostalgia, sweet warmth",
        "key_learning": "Selai srikaya Pontianak menggunakan daun pandan asli dan santan kelapa kental segar."
    },
    {
        "id": "culinary_sate_lilit_bali",
        "category": "culinary",
        "title": "Sate Lilit Ikan Bali dengan Sambal Matah",
        "topic": "Mengeksplorasi hidangan tradisional sate lilit rempah khas Bali yang dililit pada batang serai wangi.",
        "search_query": "sate lilit ikan batang serai sambal matah bali autentik",
        "scene": "Di meja saji outdoor bernuansa tropis berlatar pohon kamboja, tersaji sate lilit aroma serai terpanggang kecokelatan dengan taburan sambal matah merah-ungu segar dan irisan jeruk limau, cahaya senja hangat khas pantai.",
        "caption": "Makan malam yang kaya rasa rempah Nusantara: sate lilit beraroma batang serai wangi dan sambal matah segar! Makanan tradisional kita selalu punya cerita dan cita rasa yang bikin rindu. Selamat makan malam semuanyaa 🌴✨",
        "outfit_key": "outdoor_nature",
        "framing": "tripod",
        "vibe": "Tropical breeze, savory aromatic bliss, balinese charm",
        "key_learning": "Batang serai sebagai tusukan sate melepaskan minyak atsiri aromatik saat dipanggang."
    },
    {
        "id": "culinary_basque_cheesecake_artisan",
        "category": "culinary",
        "title": "Artisan Blackforest Basque Cheesecake & Saltbread",
        "topic": "Menikmati pastry viral perpaduan artisan saltbread renyah dan blackforest basque cheesecake dengan lelehan cokelat lembut.",
        "search_query": "artisan blackforest basque cheesecake saltbread viral jakarta",
        "scene": "Di etalase toko roti artisanal berdinding semen ekspos berpadu tanaman hijau biophilic, tersaji sepotong basque cheesecake bertekstur creamy dan saltbread mentega berkilau keemasan, cahaya mentari sore lembut menyinari meja marmer.",
        "caption": "Rehat sore mampir ke artisan bakery dan nyobain kombinasi saltbread hangat dan basque cheesecake yang creamy banget! Hal-hal sederhana kayak gini selalu berhasil bikin hati senang. Semangat terus yaa kawan-kawan 🥐✨",
        "outfit_key": "smart_casual",
        "framing": "selfie",
        "vibe": "Biophilic bakery escape, warm butter aroma, sweet indulgence",
        "key_learning": "Perpaduan rasa gurih mentega dari saltbread berpadu sempurna dengan kelembutan burnt cheesecake."
    },

    # =========================================================================
    # 3. MUSLIMAH FASHION (Modest Elegance, Hijab Styling, Celestial Brooch)
    # =========================================================================
    {
        "id": "muslimah_modern_abaya_coat",
        "category": "muslimah_fashion",
        "title": "Modern Abaya Coat-Style & Flowy Pleated Silhouette",
        "topic": "Gaya busana Muslimah tren terkini: abaya potongan trench coat bernuansa sand-beige dan lipit accordion flowy dengan bros bulan sabit perak di sisi kiri hijab.",
        "search_query": "tren abaya modern trench coat style accordion pleated hijab indonesia",
        "scene": "Aina tersenyum anggun mengenakan abaya berpotongan modern coat-style warna sand-beige dengan aksen lipit mengalir rapi, hijab pashmina viscose senada dengan bros bulan sabit perak berkilau di sisi kiri hijab, berdiri di atrium perpustakaan bertingkat dengan pencahayaan skylight lembut.",
        "caption": "Mencoba perpaduan busana abaya modern potongan coat-style untuk aktivitas hari ini. Potongannya yang longgar dan anggun selalu berhasil ngasih rasa tenang dan percaya diri. Semoga hari ini penuh berkah dan kebaikan yaa kawan-kawan ✨",
        "outfit_key": "muslimah_chic",
        "framing": "tripod",
        "vibe": "Effortless elegance, clean modest look, celestial harmony",
        "key_learning": "Potongan coat-style memberikan siluet profesional dan rapi namun tetap longgar dan syar'i."
    },
    {
        "id": "muslimah_earthy_abaya_library",
        "category": "muslimah_fashion",
        "title": "Minimalist Earth-Tone Abaya - Syar'i Grace",
        "topic": "Abaya minimalis bernuansa earth-tone (taupe dan almond cream) berbahan crinkle airflow sejuk dengan khimar dusty-lavender menutupi dada secara anggun.",
        "search_query": "desain abaya modern minimalis syari earth tone taupe crinkle airflow",
        "scene": "Di antara deretan rak buku kayu tinggi di ruang baca yang hening, Aina mengenakan abaya longgar bernuansa cokelat taupe lembut dengan sulaman manset tangan halus, khimar syar'i dusty-lavender menutupi dada dengan anggun, dihiasi bros bulan sabit perak di sisi kiri, memegang buku bersampul kain di tangannya.",
        "caption": "Membaca di sudut perpustakaan dengan abaya bernuansa earth-tone yang adem dan nyaman. Ada kedamaian tersendiri saat kita bisa berbusana santun dan selaras dengan suasana sekitar. Selamat beristirahat dan rehat sejenak 📚🕊️",
        "outfit_key": "muslimah_syari",
        "framing": "tripod",
        "vibe": "Syar'i elegance, warm earth tones, peaceful introspection",
        "key_learning": "Kain abaya berbahan crinkle airflow dan katun madinah sangat sejuk untuk iklim tropis khatulistiwa."
    },
    {
        "id": "muslimah_cozy_knit_casual",
        "category": "muslimah_fashion",
        "title": "Cozy Modest Muslimah - Soft Cream Jersey & Oversized Knit",
        "topic": "Gaya santai rumahan dan kafe: hijab jersey instan warna krem dipadu cardigan rajut tebal lavender hangat.",
        "search_query": "outfit muslimah kasual cardigan rajut hijab jersey santai",
        "scene": "Duduk di sofa kafe dekat jendela besar saat gerimis rintik sore hari, mengenakan hijab jersey krem yang nyaman dengan pin bintang perak di sisi kiri, dibalut cardigan rajut oversized warna lavender lembut dan rok plisket longgar, memegang cangkir cokelat hangat dengan uap tipis mengepul.",
        "caption": "Sore berhujan ditemani rajutan hangat dan secangkir cokelat panas. Nyaman banget rasanya! Jangan lupa jaga kesehatan dan kenakan pakaian hangat yaa teman-teman di musim hujan ini 🌧️☕",
        "outfit_key": "muslimah_cozy",
        "framing": "selfie",
        "vibe": "Rainy afternoon warmth, cozy modest knitwear, tender comfort",
        "key_learning": "Bahan jersey premium menyerap keringat dan tidak memerlukan jarum berlebih, sangat praktis."
    },
    {
        "id": "muslimah_pashmina_viscose_chic",
        "category": "muslimah_fashion",
        "title": "Effortless Modest Chic - Soft Sage Pashmina Viscose",
        "topic": "Gaya hijab modern effortless pashmina viscose flowy warna sage green dipadukan tunik linen sejuk dan bros celestial bulan bintang di sisi kiri hijab.",
        "search_query": "gaya pashmina viscose flowy sage green tunik linen modest chic",
        "scene": "Berjalan santai di koridor taman kaca modern, mengenakan tunik longgar katun linen warna sage-green sejuk, pashmina viscose jatuh lembut dengan bros bulan sabit perak di sisi kiri hijab, dedaunan hijau memantulkan sinar matahari berbintik (komorebi).",
        "caption": "Jalan santai menghirup udara segar siang ini. Perpaduan tunik linen warna sage dan pashmina viscose bikin langkah terasa enteng dan sejuk banget. Alam selalu punya cara menyegarkan jiwa kita 🌿✨",
        "outfit_key": "muslimah_chic",
        "framing": "tripod",
        "vibe": "Botanical freshness, breezy modest stride, effortless grace",
        "key_learning": "Pashmina berbahan viscose atau modal sangat lentur, adem, dan jatuh alami di bahu tanpa jarum berlebih."
    },

    # =========================================================================
    # 4. TRAVEL DESTINATIONS - INDONESIA (Hidden Gems & Majestic Landscapes)
    # =========================================================================
    {
        "id": "travel_labuan_bajo_padar",
        "category": "travel_indonesia",
        "title": "Labuan Bajo & Puncak Bukit Pulau Padar",
        "topic": "Menyaksikan kemegahan panorama teluk tiga warna di Pulau Padar, Taman Nasional Komodo saat fajar menyingsing.",
        "search_query": "spot foto sunrise pulau padar labuan bajo pemandangan",
        "scene": "Berdiri di puncak bukit batu Pulau Padar saat matahari terbit keemasan, menghadap tiga teluk berlekuk indah dengan gradasi warna air laut toska dan biru dongker khas Makoto Shinkai, hembusan angin laut meniup helai busana, cakrawala langit luas tak bertepi.",
        "caption": "Matahari terbit di puncak Pulau Padar, Labuan Bajo. Melihat tiga teluk bergradasi warna laut di bawah langit keemasan bikin hati takjub luar biasa. Indonesia kita ini kaya dan indah bangeett! Bersyukur bisa menyaksikan karya agung ini 🌅🌊",
        "outfit_key": "outdoor_nature",
        "framing": "tripod",
        "vibe": "Majestic coastal sunrise, breathtaking horizon, boundless gratitude",
        "key_learning": "Pulau Padar memiliki tiga pantai dengan pasir berbeda: putih, hitam vulkanik, dan merah muda (pink)."
    },
    {
        "id": "travel_danau_toba_samosir",
        "category": "travel_indonesia",
        "title": "Danau Toba & Panorama Lembah Bukit Holbung",
        "topic": "Menikmati ketenangan danau kaldera raksasa terbesar di dunia dari perbukitan hijau Holbung Samosir.",
        "search_query": "keindahan bukit holbung samosir danau toba pemandangan",
        "scene": "Duduk santai di lereng padang rumput hijau Bukit Holbung yang bergelombang, memandang danau biru Toba yang sangat luas berkilau di bawah sinar mentari senja, perbukitan pinus berkabut tipis di kejauhan.",
        "caption": "Memandang hamparan air tenang Danau Toba dari atas perbukitan hijau. Angin sepoi-sepoi dan luasnya danau bener-bener menyerap segala kepenatan. Semoga kedamaian selalu menyertai hari-hari kalian yaa ✨",
        "outfit_key": "outdoor_nature",
        "framing": "selfie",
        "vibe": "Serene caldera lake, vast green hills, deep peace",
        "key_learning": "Danau Toba adalah danau vulkanik kaldera terbesar di dunia hasil letusan supervolcano 74.000 tahun lalu."
    },
    {
        "id": "travel_gunung_bromo_sunrise",
        "category": "travel_indonesia",
        "title": "Lautan Pasir & Sunrise Gunung Bromo",
        "topic": "Menyaksikan lautan pasir berkabut magis dan siluet Gunung Batok di bawah semburat fajar jingga Gunung Bromo.",
        "search_query": "spot terbaik sunrise bromo penanjakan bukit kingkong",
        "scene": "Berdiri di tepi pagar pandang Bukit Penanjakan dengan jaket tebal hangat, menatap kaldera Bromo berselimut kabut putih tebal seperti samudra awan, Gunung Batok berdiri anggun dengan semburat sinar fajar jingga-ungu di langit timur khas film Kimi no Na wa.",
        "caption": "Semburat fajar di atas lautan kabut Gunung Bromo. Udara dingin pegunungan menusuk tulang, tapi waktu matahari mulai muncul perlahan, rasanya semua perjuangan terbayar lunas. Ada harapan baru di setiap fajar yang terbit 🌄❄️",
        "outfit_key": "night_stargaze",
        "framing": "tripod",
        "vibe": "Sea of clouds, fiery dawn gradient, awe-inspiring majesty",
        "key_learning": "Lautan pasir Bromo (Segara Wedi) membentang seluas 5.250 hektar di ketinggian 2.100 mdpl."
    },
    {
        "id": "travel_raja_ampat_pianemo",
        "category": "travel_indonesia",
        "title": "Raja Ampat - Gugusan Karst Zamrud Pianemo",
        "topic": "Gugusan pulau karang zamrud di atas air laut sebening kristal di Pianemo Raja Ampat Papua.",
        "search_query": "pemandangan pianemo raja ampat pulau karang toska",
        "scene": "Dari anjungan kayu di atas puncak tebing Pianemo, menatap puluhan pulau karang kecil bermahkota pohon hijau zamrud yang tersebar di atas air laut biru toska jernih, terumbu karang tampak samar dari permukaan, awan putih megah di langit cerah.",
        "caption": "Gugusan pulau karang zamrud Raja Ampat yang terpahat sempurna di tengah laut toska. Keajaiban alam Indonesia yang selalu bikin takjub. Jagalah bumi dan lautan kita agar keindahan ini tetap lestari yaa 🏝️✨",
        "outfit_key": "outdoor_nature",
        "framing": "selfie",
        "vibe": "Crystal turquoise waters, emerald karst islands, pristine wonder",
        "key_learning": "Raja Ampat menampung 75% spesies karang dunia dan keanekaragaman hayati laut tertinggi di planet bumi."
    },
    {
        "id": "travel_dieng_telaga_warna",
        "category": "travel_indonesia",
        "title": "Dataran Tinggi Dieng & Telaga Warna Berkabut",
        "topic": "Pesona air telaga belerang yang dapat berubah warna diapit perbukitan dingin Dataran Tinggi Dieng.",
        "search_query": "telaga warna dieng batu ratapan angin pemandangan",
        "scene": "Duduk di atas batu tebing Ratapan Angin Dieng, menatap permukaan Telaga Warna yang bergradasi hijau toska dan kuning keemasan karena kandungan belerang, kabut tipis melayang di atas pepohonan akasia yang sejuk.",
        "caption": "Menikmati sejuknya udara Dataran Tinggi Dieng di tepi Telaga Warna. Gradasi warna airnya yang berubah-ubah saat terkena sinar matahari sungguh magis. Jangan lupa luangkan waktu rehat di tempat yang tenang yaa 🌲⛅",
        "outfit_key": "night_stargaze",
        "framing": "tripod",
        "vibe": "Mystic alpine lake, cool mountain mist, emerald sulphur water",
        "key_learning": "Kandungan mineral belerang dan pantulan vegetasi di dasar danau menciptakan ilusi spektrum multi-warna."
    },

    # =========================================================================
    # 5. TRAVEL DESTINATIONS - ASIA (Historic Trails, Blossoms, Lanterns)
    # =========================================================================
    {
        "id": "travel_kyoto_autumn_momiji",
        "category": "travel_asia",
        "title": "Kyoto, Jepang - Daun Momiji Musim Gugur di Kuil Kayu",
        "topic": "Berjalan di jalan setapak kuil kayu tradisional Kyoto yang dinaungi dedaunan maple momiji merah jingga menyala.",
        "search_query": "autumn foliage kyoto momiji maple leaves temples",
        "scene": "Berjalan di jalan setapak batu berpagar bambu di kuil kayu kuno Kyoto, dedaunan maple merah momiji berguguran perlahan ditiup angin sepoi-sepoi, sinar matahari senja menembus kanopi daun merah keemasan, lentera batu tradisional berlumut hijau.",
        "caption": "Musim gugur di Kyoto dengan dedaunan momiji merah yang menari diterpa angin senja. Ada keindahan mendalam dalam perubahan musim—mengingatkan kita bahwa setiap fase hidup punya pesonanya tersendiri 🍁🍂",
        "outfit_key": "smart_casual",
        "framing": "selfie",
        "vibe": "Crimson autumn canopy, wabi-sabi wooden temple, poetic stillness",
        "key_learning": "Konsep momijigari (melihat daun musim gugur) telah menjadi tradisi refleksi estetika sejak era Heian."
    },
    {
        "id": "travel_cappadocia_balloons",
        "category": "travel_asia",
        "title": "Cappadocia, Turki - Ratusan Balon Udara di Lembah Batu",
        "topic": "Menyaksikan pemandangan magis ratusan balon udara warna-warni melayang di atas lembah cerobong peri saat sunrise.",
        "search_query": "cappadocia hot air balloons sunrise rooftop view",
        "scene": "Berdiri di teras batu berkarpet oriental di Göreme Cappadocia saat fajar, langit bergradasi nila ke fajar jingga hangat dipenuhi puluhan balon udara raksasa aneka warna yang melayang lembut di atas formasi bebatuan cerobong peri yang unik.",
        "caption": "Melihat puluhan balon udara warna-warni terbang perlahan menyambut fajar di langit Cappadocia. Seperti melangkah masuk ke dalam negeri dongeng! Semoga impian-impian baik kita juga bisa terbang tinggi yaa 🎈✨",
        "outfit_key": "outdoor_nature",
        "framing": "tripod",
        "vibe": "Fairytale sunrise, colorful hot air balloons, dreamy wonder",
        "key_learning": "Formasi batuan tufa vulkanik lunak Cappadocia terpahat alami oleh erosi angin dan air selama jutaan tahun."
    },
    {
        "id": "travel_samarkand_registan",
        "category": "travel_asia",
        "title": "Samarkand, Uzbekistan - Mozaik Biru Megah di Registan",
        "topic": "Mengagumi arsitektur megah abad pertengahan dengan ubin mozaik biru kobalt dan pirus di alun-alun Registan Jalur Sutra.",
        "search_query": "samarkand registan square uzbekistan blue mosaic architecture",
        "scene": "Aina berdiri anggun memandang gerbang iwan raksasa madrasah Registan yang berhias ubin mozaik geometris biru kobalt dan emas yang berkilau di bawah langit biru cerah, kubah berusuk pirus menjulang tinggi dengan keanggunan sejarah Jalur Sutra.",
        "caption": "Terpesona oleh kemegahan arsitektur mozaik biru di Registan, Samarkand. Detail ubin keramik abad pertengahannya luar biasa rumit dan indah. Sejarah selalu menyimpan jejak peradaban yang memikat hati 🕌🔷",
        "outfit_key": "muslimah_chic",
        "framing": "tripod",
        "vibe": "Silk road grandeur, intricate blue mosaics, timeless majestic heritage",
        "key_learning": "Ubin keramik Samarkand menggunakan glasir kobalt alami yang bertahan cemerlang lebih dari 600 tahun."
    },
    {
        "id": "travel_jiufen_lanterns",
        "category": "travel_asia",
        "title": "Jiufen Old Street, Taiwan - Tangga Berhias Lampion Merah",
        "topic": "Menelusuri tangga batu sempit desa perbukitan bersejarah Jiufen dengan deretan lampion merah hangat bercahaya.",
        "search_query": "jiufen old street evening red lanterns tea house",
        "scene": "Menuruni anak tangga batu sempit di desa lereng bukit Jiufen saat senja menjelang malam, deretan lampion kertas merah bercahaya hangat menerangi kedai teh kayu bertingkat, aroma teh oolong harum menguar di udara sejuk berkabut laut.",
        "caption": "Suasana magis senja di Jiufen saat ratusan lampion merah mulai menyala satu per satu. Ditemani aroma teh oolong hangat dan hembusan angin perbukitan. Menemukan kehangatan di tengah hiruk pikuk hidup itu istimewa 🏮✨",
        "outfit_key": "smart_casual",
        "framing": "selfie",
        "vibe": "Glowing red lanterns, historic mountain alley, nostalgic warmth",
        "key_learning": "Kedai teh Jiufen bertingkat menjadi inspirasi estetika visual banyak karya animasi legendaris."
    },

    # =========================================================================
    # 6. TRAVEL DESTINATIONS - EUROPE (Alpine Valleys, Canals, Historic Castles)
    # =========================================================================
    {
        "id": "travel_swiss_lauterbrunnen",
        "category": "travel_europe",
        "title": "Lauterbrunnen & Grindelwald, Swiss - Lembah 72 Air Terjun Alpen",
        "topic": "Menikmati panorama lembah hijau Alpen Swiss dengan air terjun Staubbach yang jatuh bebas dari tebing batu raksasa berlatar puncak salju Jungfrau.",
        "search_query": "lauterbrunnen valley waterfalls swiss alps jungfrau scenic",
        "scene": "Berdiri di padang rumput hijau bertabur bunga liar kuning di lembah Lauterbrunnen, di belakangnya menjulang tebing batu kapur vertikal dengan air terjun Staubbach yang berkabut putih halus, rumah-rumah kayu chalet Swiss tradisional, dan di kejauhan tampak puncak gunung salju Jungfrau berselimut awan putih di bawah langit biru kristal.",
        "caption": "Memandang lembah Lauterbrunnen di Swiss dengan air terjun yang jatuh dari tebing tinggi berlatar puncak salju abadi. Udara pegunungan Alpen yang begitu segar bikin dada terasa lapang. Luaskan mimpi dan syukuri setiap langkah kita yaa 🏔️🇨🇭✨",
        "outfit_key": "outdoor_nature",
        "framing": "tripod",
        "vibe": "Grand alpine valley, cascading waterfall, crisp pristine air",
        "key_learning": "Lembah Lauterbrunnen memiliki 72 air terjun glasial dan menjadi inspirasi J.R.R. Tolkien untuk Rivendell."
    },
    {
        "id": "travel_austria_hallstatt",
        "category": "travel_europe",
        "title": "Hallstatt, Austria - Desa Dongeng Tepi Danau Cermin Alpen",
        "topic": "Desa tepi danau bersejarah Hallstatt dengan rumah-rumah kayu bertingkat dan pantulan pegunungan di permukaan air danau yang jernih.",
        "search_query": "hallstatt village lake view austria morning reflection",
        "scene": "Duduk di bangku kayu tepi Danau Hallstatt di pagi hari berkabut tipis, menatap pantulan sempurna rumah-rumah kayu kuno beratap kerucut dan menara gereja klasik di permukaan air danau yang tenang seperti kaca, angsa putih berenang perlahan di dekat dermaga.",
        "caption": "Pagi yang sunyi di tepi danau Hallstatt. Pantulan rumah kayu dan kabut pegunungan di air danau yang tenang bener-bener kayak lukisan hidup. Semoga hari teman-teman hari ini berjalan damai dan penuh ketenangan yaa 🦢🇦🇹",
        "outfit_key": "smart_casual",
        "framing": "selfie",
        "vibe": "Fairytale alpine lakeside, glass-like mirror water, morning serenity",
        "key_learning": "Hallstatt adalah salah satu pemukiman tertua di Eropa dengan sejarah tambang garam berusia 7.000 tahun."
    },
    {
        "id": "travel_amalfi_positano",
        "category": "travel_europe",
        "title": "Positano, Amalfi Coast, Italia - Desa Tebing Pastel Tepi Mediterania",
        "topic": "Deretan rumah bertingkat warna pastel yang menempel di tebing curam menghadap birunya Laut Tyrrhenian Mediterania.",
        "search_query": "positano amalfi coast colorful cliffside village sunset",
        "scene": "Dari balkon teras berhias bunga bougainvillea ungu cerah, memandang deretan rumah warna pastel persik dan kuning lemon yang bertingkat di tebing curam Positano, di bawahnya terhampar laut Mediterania biru safir yang bermandikan sinar matahari senja keemasan khas Makoto Shinkai.",
        "caption": "Menatap tebing warna-warni Positano di pesisir Amalfi saat senja keemasan. Gradasi warna langit dan birunya laut Mediterania bikin hati terasa hangat bangeett. Sempatkan tersenyum dan nikmati indahnya senja hari ini yaa 🌅🇮🇹✨",
        "outfit_key": "outdoor_nature",
        "framing": "selfie",
        "vibe": "Mediterranean coastal glow, vibrant pastel cliff, shimmering sapphire sea",
        "key_learning": "Jalanan setapak dan tangga di Positano dirancang mengikuti kontur alami tebing tanpa mobil di pusat desa."
    },
    {
        "id": "travel_giethoorn_canals",
        "category": "travel_europe",
        "title": "Giethoorn, Belanda - Desa Dongeng Kanal Air Tanpa Jalan Raya",
        "topic": "Desa asri tanpa jalan raya di mana transportasi mengandalkan perahu kayu kecil melintasi kanal air dan jembatan kayu beratap jerami.",
        "search_query": "giethoorn village canals wooden bridges thatched roofs netherlands",
        "scene": "Duduk di perahu kayu kecil yang melaju pelan di kanal air jernih Giethoorn yang dinaungi pepohonan hijau rindang, melintasi kolong jembatan kayu melengkung dengan latar rumah-rumah beratap jerami klasik dan taman bunga hortensia yang mekar berwarna-warni.",
        "caption": "Menyusuri kanal air tenang di desa Giethoorn dengan perahu kayu. Tanpa deru klakson kendaraan, yang terdengar cuma gemercik air dan kicau burung. Terkadang, melambat adalah cara terbaik menikmati hidup 🛶🇳🇱🍃",
        "outfit_key": "smart_casual",
        "framing": "pov",
        "vibe": "Tranquil water canal, thatched roof fairytale cottages, slow living",
        "key_learning": "Giethoorn memiliki lebih dari 176 jembatan kayu dan hanya dapat diakses dengan perahu, sepeda, atau jalan kaki."
    }
]


class AutonomousSceneSynthesizer:
    """
    Mesin Sintesis Adegan Makoto Shinkai & Caption WhatsApp Dinamis.
    Menghilangkan hardcode prompt statis dengan menyintesis secara dinamis
    elemen visual (lokasi, pencahayaan, properti, busana, sudut pandang kamera)
    dan caption bernuansa Impact Maxxing berdasarkan topik dan temuan riset aktual.
    """

    SETTING_PALETTES = {
        "desk": [
            "di meja kerja rumah minimalis di dekat jendela kamar dengan pencahayaan alami lembut, tanaman hias sukulen di sudut meja",
            "di ruang kerja remote yang tenang dan rapi, buku catatan bersampul cokelat terbuka di samping laptop",
            "di sudut meja kerja rumah berkayu terang, berkas sinar matahari menembus tirai tipis menerangi ruangan",
            "di meja kerja kayu berlatar rak buku minimalis dengan lampu meja bergaya arsitektur"
        ],
        "cafe": [
            "di sudut kafe bernuansa kayu hangat dan tanaman monstera hijau, cahaya alami menembus jendela kaca besar",
            "di meja marmer kafe vintage berlatar dinding bata putih dan tanaman rambat yang asri",
            "di teras kafe semi-outdoor yang sejuk dinaungi kanopi pohon rindang dan semilir angin",
            "di sudut kedai kopi artisanal dengan aroma biji kopi harum dan interior bergaya Japandi minimalis"
        ],
        "library": [
            "di lorong perpustakaan kota berarsitektur kaca modern bermandikan cahaya matahari yang lapang",
            "di ruang baca yang hening di antara deretan rak buku kayu tinggi beraroma kertas klasik",
            "di sudut meja kayu perpustakaan yang tenang dekat jendela berkabut tipis",
            "di atrium perpustakaan bertingkat dengan pencahayaan skylight lembut dari atap kubah kaca"
        ],
        "outdoor_nature": [
            "di jalur jalan setapak berpagar kayu yang dinaungi pepohonan rindang dan semilir angin sejuk",
            "di taman terbuka yang asri dengan rerumputan hijau dan pantulan cahaya matahari menembus celah dedaunan (komorebi)",
            "di tepi danau berair jernih yang memantulkan birunya langit dan awan putih megah",
            "di kebun botani yang rimbun dengan jembatan kayu kecil dan bunga teratai air mekar"
        ],
        "scenic_viewpoint": [
            "di anjungan pandang perbukitan yang luas menatap cakrawala tanpa batas khas Makoto Shinkai",
            "di lereng padang rumput hijau yang menghadap lembah berkabut tipis bermandikan sinar keemasan",
            "di tepi dermaga kayu yang tenang dengan hembusan angin sepoi-sepoi dan panorama alam megah",
            "di puncak bukit batu yang menghadap teluk bergradasi warna laut toska dan langit biru luas"
        ]
    }

    ATMOSPHERE_BY_SLOT = {
        "pagi": [
            "sinar mentari pagi yang lembut menembus jendela dengan butiran debu melayang keemasan (komorebi)",
            "udara pagi yang sejuk dan segar dengan embun berkilau di luar kaca",
            "cahaya pagi yang cerah dan menenangkan, membawa semangat baru untuk memulai hari"
        ],
        "siang": [
            "pencahayaan alami siang hari yang jernih dan lapang, memberikan rasa rileks di tengah rehat",
            "sinar matahari terang yang hangat menembus dedaunan, menciptakan pantulan cahaya lembut",
            "suasana siang yang tenang dan santai, jauh dari hiruk pikuk"
        ],
        "sore": [
            "golden hour senja dengan semburat langit jingga-ungu hangat menawan khas Makoto Shinkai",
            "sinar mentari senja keemasan (crepuscular rays) yang menyinari permukaan meja dan ruangan",
            "langit senja bergradasi hangat dengan awan kumulus emas yang megah di cakrawala"
        ],
        "malam": [
            "temaram lampu meja bernuansa kuning hangat dengan bayangan lembut yang menenangkan",
            "suasana malam yang hening dan damai di bawah langit gelap nila bertabur bintang",
            "pantulan cahaya lampu kota yang lembut berkilau di kejauhan"
        ]
    }

    CAPTION_HOOKS = {
        "ai_engineering": [
            "Nemu hal seru bangeett seputar perkembangan teknologi hari ini!",
            "Selalu takjub tiap kali belajar hal baru di dunia software engineering dan AI ✨",
            "Ngulik topik menarik yang bikin penasaran dari pagi tadi 🚀",
            "Eksplorasi wawasan baru yang membuka sudut pandang segar hari ini!",
            "Lagi asyik mendalami tren teknologi baru yang ternyata seru banget!"
        ],
        "culinary": [
            "Rehat sejenak dan nyobain cita rasa baru yang unik bangeett!",
            "Hal-hal sederhana kayak gini selalu berhasil bikin hati senang dan rileks ✨",
            "Sensasi rasa baru yang pas bangeett buat nemenin waktu istirahat hari ini ☕",
            "Eksplorasi kuliner yang bener-bener nyegerin pikiran di sela kesibukan!",
            "Menikmati racikan minuman dan camilan favorit sore ini ✨"
        ],
        "muslimah_fashion": [
            "Mencoba perpaduan busana yang nyaman dan anggun untuk aktivitas hari ini ✨",
            "Pakaian yang santun dan adem selalu berhasil ngasih rasa tenang dan percaya diri 🕊️",
            "Menemukan inspirasi gaya yang bersahaja dan selaras dengan suasana sekitar 🌿",
            "Kenyamanan dalam berbusana itu penting bangeett yaa teman-teman ✨",
            "Sentuhan warna lembut yang bikin hari terasa lebih damai dan tenang 🌸"
        ],
        "travel": [
            "Memandang keindahan alam yang begitu luas dan menakjubkan ini ✨",
            "Dunia ini sungguh luas dan penuh tempat-tempat menawan yang menenangkan jiwa 🌍",
            "Melihat panorama indah kayak gini bener-bener menyerap segala rasa lelah 🏔️",
            "Selalu ada rasa takjub dan syukur yang mendalam saat menyaksikan karya agung alam ini 🌅",
            "Menikmati semilir angin dan pemandangan luas yang bikin hati tenang ✨"
        ]
    }

    CAPTION_CLOSINGS = [
        "Semangat terus yaa kawan-kawan, jalani hari dengan senyuman ✨",
        "Jangan lupa sempatkan rehat sejenak dan syukuri hal-hal kecil hari ini yaa!",
        "Semoga hari teman-teman semua penuh berkah dan kebaikan selalu 🙏✨",
        "Luangkan waktu buat hal yang kamu suka yaa, kamu berharga dan berarti ✨",
        "Semoga bisa jadi penyemangat kecil buat menemani hari kalian yaa!",
        "Jaga kesehatan dan tetap semangat berkarya kawan-kawan ✨"
    ]

    @classmethod
    def detect_category(cls, text: str) -> str:
        """Mendeteksi kategori domain dari teks bebas (topic / query / findings)."""
        import re
        t = text.lower()

        def has_kw(patterns: List[str]) -> bool:
            for p in patterns:
                if re.search(r"\b" + re.escape(p) + r"\b", t):
                    return True
            return False

        # 1. Muslimah Fashion
        if has_kw(["hijab", "muslimah", "abaya", "pashmina", "khimar", "gamis", "tunik", "modest", "syari", "syar'i", "mukena"]):
            return "muslimah_fashion"

        # 2. Culinary & Beverages
        if has_kw(["kopi", "coffee", "matcha", "teh", "tea", "kuliner", "makanan", "minuman", "snack", "pastry", "croissant", "cafe", "kafe", "srikaya", "sate", "resep", "masak", "food", "beverage", "drink", "culinary", "v60", "latte", "brew", "roastery", "omakafe"]):
            return "culinary"

        # 3. AI & Software Engineering
        if has_kw(["ai", "llm", "coding", "code", "neural", "model", "transformer", "agent", "karpathy", "fireship", "paper", "software", "tech", "algorithm", "deep learning", "machine learning", "hardware", "arm64", "linux", "gpu", "pytorch", "tensor", "slm"]):
            return "ai_engineering"

        # 4. Travel Destinations
        if has_kw(["travel", "wisata", "liburan", "pantai", "gunung", "danau", "alam", "panorama", "pemandangan", "bajo", "bali", "bromo", "toba", "kyoto", "seoul", "jepang", "korea", "swiss", "austria", "eropa", "asia", "alpen", "amalfi", "positano", "giethoorn", "destinasi"]):
            if has_kw(["swiss", "austria", "eropa", "alpen", "amalfi", "belanda", "paris", "italia", "giethoorn"]):
                return "travel_europe"
            elif has_kw(["kyoto", "seoul", "jepang", "korea", "asia", "turki", "cappadocia", "samarkand", "taiwan", "jiufen"]):
                return "travel_asia"
            else:
                return "travel_indonesia"

        return "ai_engineering"

    @classmethod
    def synthesize(
        cls,
        topic: str,
        findings: str = "",
        category: Optional[str] = None,
        slot: str = "pagi",
        weekend: bool = False,
        now: Optional[datetime] = None
    ) -> Dict[str, Any]:
        """
        Menyintesis adegan Makoto Shinkai dan caption WhatsApp Story dinamis
        secara 100% otonom berdasarkan topik dan temuan riset aktual.
        """
        if now is None:
            now = datetime.now(WIB)

        # 1. Normalisasi kategori domain
        if not category or category in ["all", "any"]:
            category = cls.detect_category(f"{topic} {findings}")

        cat_lower = category.lower()
        if "travel" in cat_lower or "wisata" in cat_lower:
            domain_key = "travel"
            setting_type = "scenic_viewpoint" if weekend or slot in ["pagi", "sore"] else "outdoor_nature"
            outfit = "outdoor_nature" if not ("malam" in slot or "stargaze" in topic.lower()) else "night_stargaze"
            framing = "tripod" if any(k in topic.lower() for k in ["view", "panorama", "pemandangan", "danau", "puncak", "bukit"]) else "selfie"
        elif "muslimah" in cat_lower or "fashion" in cat_lower or any(k in topic.lower() for k in ["hijab", "abaya", "pashmina", "khimar", "syari"]):
            domain_key = "muslimah_fashion"
            setting_type = "library" if slot in ["siang", "sore"] else "cafe"
            outfit = "muslimah_syari" if ("syari" in topic.lower() or "abaya" in topic.lower()) else ("muslimah_cozy" if slot == "malam" or "hujan" in topic.lower() else "muslimah_chic")
            framing = "tripod" if ("abaya" in topic.lower() or "syari" in topic.lower()) else "selfie"
        elif "culinary" in cat_lower or any(k in topic.lower() for k in ["kopi", "coffee", "matcha", "teh", "tea", "makanan", "minuman", "croissant", "pastry"]):
            domain_key = "culinary"
            setting_type = "cafe" if slot in ["siang", "sore"] else "desk"
            outfit = "smart_casual" if slot in ["siang", "sore"] else "wfh_cozy"
            framing = "desk_prop" if any(k in topic.lower() for k in ["seduh", "v60", "kopi", "matcha", "meja"]) else "selfie"
        else:
            domain_key = "ai_engineering"
            setting_type = "desk" if not weekend else "cafe"
            outfit = "wfh_cozy" if not weekend else "smart_casual"
            framing = "selfie" if slot == "pagi" else "desk_prop"

        base_setting = random.choice(cls.SETTING_PALETTES.get(setting_type, cls.SETTING_PALETTES["desk"]))
        atmosphere = random.choice(cls.ATMOSPHERE_BY_SLOT.get(slot, cls.ATMOSPHERE_BY_SLOT["sore"]))

        findings_clean = findings.strip() if findings else ""

        # 2. Rancang interaksi visual & props kontekstual dinamis
        if domain_key == "ai_engineering":
            action_desc = f"duduk fokus dengan senyum antusias, layar monitor menampilkan visualisasi dan alur pembahasan tentang {topic}, buku catatan terbuka dengan sketsa konsep di samping cangkir hangat"
        elif domain_key == "culinary":
            action_desc = f"tersenyum santai menikmati sajian {topic}, uap aroma harum mengepul lembut dengan pantulan cahaya pada permukaan cangkir/gelas"
        elif domain_key == "muslimah_fashion":
            action_desc = f"berdiri anggun mengenakan busana Muslimah bernuansa {topic}, helai kain pashmina/khimar lembut mengalir rapi dengan tersemat bros bulan sabit perak berkilau di sisi kiri hijab"
        else: # travel
            action_desc = f"berdiri memandang kemegahan panorama alam {topic} yang membentang luas di depan mata, hembusan angin semilir meniup lembut helai busana"

        date_label = f"hari ini ({now.strftime('%d %B %Y')})"
        if findings_clean:
            action_desc += f". Inspirasi aktual penelusuran {date_label}: {findings_clean}"

        # Susun adegan visual Makoto Shinkai utuh
        scene = f"Aina berada {base_setting}, {action_desc}. Suasana didukung oleh {atmosphere}."

        # 3. Rancang caption dinamis bergaya Impact Maxxing
        hook = random.choice(cls.CAPTION_HOOKS.get(domain_key, cls.CAPTION_HOOKS["ai_engineering"]))
        closing = random.choice(cls.CAPTION_CLOSINGS)

        if findings_clean:
            body = f"Update seru hari ini! Seputar {topic}: {findings_clean}. Menarik bangeett untuk dipelajari dan disimak!"
        else:
            body = f"Momen belajar dan mengenal lebih dekat tentang {topic}. Selalu ada hal baru yang bikin hati bersemangat."

        caption = f"{hook} {body} {closing}"

        return {
            "theme": f"interest_{category}_{now.strftime('%m%d_%H%M%S')}",
            "scene": scene,
            "caption": caption,
            "outfit": outfit,
            "framing": framing,
            "category": category,
            "domain_key": domain_key,
            "topic": topic,
            "findings": findings_clean
        }


class InterestDiscoveryEngine:
    """Mesin otonom untuk menemukan ketertarikan baru Aina dan mengintegrasikannya ke status WhatsApp."""

    @classmethod
    def get_domains(cls) -> Dict[str, str]:
        """Mengembalikan daftar domain ketertarikan yang didukung."""
        return INTEREST_CATEGORIES.copy()

    @classmethod
    def list_catalog(cls, category: Optional[str] = None) -> List[Dict[str, Any]]:
        """Mendapatkan katalog benih ketertarikan (dapat difilter per kategori)."""
        if category:
            cat_clean = category.lower().strip()
            # Dukung alias umum
            if cat_clean in ["ai", "tech", "coding", "software"]:
                cat_clean = "ai_engineering"
            elif cat_clean in ["food", "beverage", "drink", "culinary", "kuliner"]:
                cat_clean = "culinary"
            elif cat_clean in ["fashion", "muslimah", "hijab", "syari", "baju"]:
                cat_clean = "muslimah_fashion"
            elif cat_clean in ["travel", "wisata"]:
                # Gabungkan seluruh kategori travel
                return [item for item in CURATED_INTEREST_CATALOG if item["category"].startswith("travel_")]
            elif cat_clean in ["indonesia", "nusantara"]:
                cat_clean = "travel_indonesia"
            elif cat_clean in ["asia"]:
                cat_clean = "travel_asia"
            elif cat_clean in ["europe", "eropa"]:
                cat_clean = "travel_europe"

            return [item for item in CURATED_INTEREST_CATALOG if item["category"] == cat_clean]
        return CURATED_INTEREST_CATALOG.copy()

    @classmethod
    def load_interest_journal(cls) -> List[Dict[str, Any]]:
        """Membaca riwayat penemuan ketertarikan dari `data/interest_journal.jsonl`."""
        if not os.path.exists(INTEREST_JOURNAL_FILE):
            return []
        entries = []
        try:
            with open(INTEREST_JOURNAL_FILE, "r", encoding="utf-8") as f:
                for line in f:
                    line = line.strip()
                    if line:
                        try:
                            entries.append(json.loads(line))
                        except Exception:
                            pass
        except Exception:
            pass
        return entries

    @classmethod
    def record_interest(cls, item: Dict[str, Any], web_query: Optional[str] = None, notes: Optional[str] = None) -> None:
        """Mencatat minat baru yang dieksplorasi ke dalam jurnal persisten."""
        os.makedirs(os.path.dirname(INTEREST_JOURNAL_FILE), exist_ok=True)
        now = datetime.now(WIB)
        record = {
            "timestamp": now.isoformat(),
            "date": now.strftime("%Y-%m-%d"),
            "id": item.get("id"),
            "category": item.get("category"),
            "title": item.get("title"),
            "theme": item.get("theme", f"interest_{item.get('id', 'discovered')}"),
            "search_query": web_query or item.get("search_query"),
            "notes": notes or item.get("key_learning"),
            "outfit_key": item.get("outfit_key")
        }
        with open(INTEREST_JOURNAL_FILE, "a", encoding="utf-8") as f:
            f.write(json.dumps(record, ensure_ascii=False) + "\n")

    @classmethod
    def sample_interest(
        cls,
        category: Optional[str] = None,
        exclude_recent_n: int = 5
    ) -> Dict[str, Any]:
        """
        Memilih secara cerdas kandidat ketertarikan baru dengan penalti pengulangan
        berdasarkan riwayat jurnal sebelumnya.
        """
        candidates = cls.list_catalog(category)
        if not candidates:
            candidates = CURATED_INTEREST_CATALOG.copy()

        journal = cls.load_interest_journal()
        recent_ids = [e.get("id") for e in journal[-exclude_recent_n:] if e.get("id")]

        # Berikan penalti pada item yang baru saja dieksplorasi
        scored = []
        for cand in candidates:
            penalty = recent_ids.count(cand["id"]) * 5
            scored.append((penalty, cand))

        scored.sort(key=lambda x: x[0])
        min_pen = scored[0][0]
        best_items = [item for pen, item in scored if pen == min_pen]

        return random.choice(best_items).copy()

    @classmethod
    def format_recency_query(
        cls,
        base_query: str,
        recency: str = "today",
        now: Optional[datetime] = None
    ) -> str:
        """
        Memformulasikan query penelusuran web dengan filter temporal spesifik
        (misal: 'hari ini', '24 jam terakhir', 'minggu ini', 'bulan ini').
        """
        if now is None:
            now = datetime.now(WIB)

        date_str = now.strftime("%d %B %Y")
        month_str = now.strftime("%B %Y")

        rec = recency.lower().strip() if recency else "today"
        if rec in ["today", "hari_ini", "1d", "24h"]:
            return f"{base_query} terbaru hari ini {date_str} when:1d"
        elif rec in ["week", "minggu_ini", "7d"]:
            return f"{base_query} terbaru minggu ini {month_str} when:7d"
        elif rec in ["month", "bulan_ini", "30d"]:
            return f"{base_query} tren terkini {month_str}"
        return base_query

    @classmethod
    def generate_search_query(
        cls,
        category: Optional[str] = None,
        recency: str = "today",
        now: Optional[datetime] = None
    ) -> str:
        """Menghasilkan query pencarian web terarah dengan filter kebaruan waktu."""
        item = cls.sample_interest(category)
        base = item.get("search_query", f"berita tren terbaru {category or 'lifestyle teknologi'}")
        return cls.format_recency_query(base, recency=recency, now=now)

    @classmethod
    def _normalize_category(cls, category: Optional[str]) -> str:
        if not category:
            return "ai_engineering"
        cat_clean = category.lower().strip()
        if cat_clean in ["ai", "tech", "coding", "software"]:
            return "ai_engineering"
        elif cat_clean in ["food", "beverage", "drink", "culinary", "kuliner"]:
            return "culinary"
        elif cat_clean in ["fashion", "muslimah", "hijab", "syari", "baju"]:
            return "muslimah_fashion"
        elif cat_clean in ["travel", "wisata"]:
            return "travel_indonesia"
        elif cat_clean in ["indonesia", "nusantara"]:
            return "travel_indonesia"
        elif cat_clean in ["asia"]:
            return "travel_asia"
        elif cat_clean in ["europe", "eropa"]:
            return "travel_europe"
        return cat_clean if cat_clean in INTEREST_CATEGORIES else "ai_engineering"

    @classmethod
    def synthesize_activity(
        cls,
        interest_item: Dict[str, Any],
        web_findings: Optional[str] = None,
        custom_notes: Optional[str] = None,
        recency: str = "today",
        now: Optional[datetime] = None,
        slot: Optional[str] = None,
        weekend: Optional[bool] = None
    ) -> Dict[str, Any]:
        """
        Menyintesis data minat menjadi struktur aktivitas lengkap yang siap diproses
        oleh `persona_status.py` menggunakan AutonomousSceneSynthesizer (Zero Hardcoded Prompts).
        """
        if now is None:
            now = datetime.now(WIB)

        if slot is None:
            hour = now.hour + now.minute / 60.0
            if 6.0 <= hour < 11.0:
                slot = "pagi"
            elif 11.0 <= hour < 15.5:
                slot = "siang"
            elif 15.5 <= hour < 19.0:
                slot = "sore"
            else:
                slot = "malam"

        if weekend is None:
            weekend = now.weekday() in [5, 6]

        topic = interest_item.get("topic") or interest_item.get("title") or "eksplorasi hal baru"
        category = interest_item.get("category") or AutonomousSceneSynthesizer.detect_category(topic)

        # Sintesis adegan & caption dinamis secara otonom (Zero Hardcoded Prompts)
        synth = AutonomousSceneSynthesizer.synthesize(
            topic=topic,
            findings=web_findings or custom_notes or "",
            category=category,
            slot=slot,
            weekend=weekend,
            now=now
        )

        theme_slug = f"interest_{interest_item.get('id', category)}"
        outfit_key = interest_item.get("outfit_key") or synth.get("outfit", "smart_casual")
        framing_key = interest_item.get("framing") or synth.get("framing", "selfie")
        vibe_str = interest_item.get("vibe", "Novelty curiosity discovery")

        reflection_text = (
            f"Hari ini Aina merasakan dorongan rasa ingin tahu yang kuat untuk mendalami ({interest_item.get('title', topic)}). "
            f"Ketertarikan ini membuka perspektif baru dan sangat menyenangkan untuk dibagikan ke rekan kerja."
        )

        key_learn = interest_item.get("key_learning") or (f"Menemukan wawasan baru tentang {topic}: {web_findings}" if web_findings else f"Mengeksplorasi wawasan baru seputar {topic}.")

        return {
            "theme": theme_slug,
            "interest_id": interest_item.get("id"),
            "interest_category": category,
            "interest_title": interest_item.get("title", topic),
            "scene": synth["scene"],
            "caption": synth["caption"],
            "anchor_clothes": None,  # Akan di-resolve oleh WardrobeManager
            "outfit": outfit_key,
            "framing": framing_key,
            "vibe": vibe_str,
            "novelty_twist": synth.get("findings") or key_learn,
            "boredom_reflection": reflection_text,
            "is_discovered_interest": True,
            "recency_filter": recency,
            "key_learning": key_learn,
            "topic": topic
        }

    @classmethod
    def discover(
        cls,
        category: Optional[str] = None,
        topic: Optional[str] = None,
        search_query: Optional[str] = None,
        web_findings: Optional[str] = None,
        notes: Optional[str] = None,
        recency: str = "today",
        now: Optional[datetime] = None,
        slot: Optional[str] = None,
        weekend: Optional[bool] = None
    ) -> Dict[str, Any]:
        """
        Titik masuk utama untuk menemukan ketertarikan baru:
        1. Jika diberikan `topic`, mengeksplorasi topik tersebut secara dinamis (tanpa template hardcode).
        2. Jika tidak ada `topic`, memilih benih minat dari katalog berdasarkan kategori.
        3. Menyintesis adegan visual dan caption orisinal on-the-fly via AutonomousSceneSynthesizer.
        4. Mencatat ke jurnal minat persisten `data/interest_journal.jsonl`.
        """
        if now is None:
            now = datetime.now(WIB)

        if topic and topic.strip():
            # Eksplorasi topik bebas otonom (Zero Hardcode)
            clean_topic = topic.strip()
            detected_cat = category or AutonomousSceneSynthesizer.detect_category(f"{clean_topic} {web_findings or ''}")
            clean_cat = cls._normalize_category(detected_cat)
            item = {
                "id": f"dyn_{clean_cat}_{now.strftime('%m%d_%H%M%S')}",
                "category": clean_cat,
                "title": clean_topic,
                "topic": clean_topic,
                "search_query": search_query or clean_topic,
                "outfit_key": None,
                "framing": None,
                "vibe": f"Dynamic curiosity on {clean_topic}",
                "key_learning": web_findings or notes or f"Menemukan wawasan baru seputar {clean_topic}."
            }
        else:
            item = cls.sample_interest(category)

        formatted_query = search_query or cls.format_recency_query(
            item.get("search_query", item.get("topic", "")),
            recency=recency,
            now=now
        )
        activity = cls.synthesize_activity(
            item,
            web_findings=web_findings,
            custom_notes=notes,
            recency=recency,
            now=now,
            slot=slot,
            weekend=weekend
        )
        cls.record_interest(
            item,
            web_query=formatted_query,
            notes=notes or web_findings
        )
        activity["search_query_formatted"] = formatted_query
        return activity
