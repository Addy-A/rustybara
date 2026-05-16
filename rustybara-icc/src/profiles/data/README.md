# ICC Profile Data

This directory is intentionally excluded from version control. The `.icc` files embedded
by `rustybara-icc` are © Adobe Systems Incorporated and are not redistributed in this
repository to comply with Adobe's license terms.

## Setup

Download the free **Adobe ICC Profiles** installer directly from Adobe:

- **Windows:** https://www.adobe.com/support/downloads/iccprofiles/iccprofiles_win.html
- **macOS:** https://www.adobe.com/support/downloads/iccprofiles/iccprofiles_mac.html

After installing, copy the profiles into the following layout:

```
rustybara-icc/src/profiles/data/
├── CMYK/
│   ├── CoatedFOGRA27.icc
│   ├── CoatedFOGRA39.icc
│   ├── CoatedGRACoL2006.icc
│   ├── JapanColor2001Coated.icc
│   ├── JapanColor2001Uncoated.icc
│   ├── JapanColor2002Newspaper.icc
│   ├── JapanColor2003WebCoated.icc
│   ├── JapanWebCoated.icc
│   ├── USWebCoatedSWOP.icc
│   ├── USWebUncoated.icc
│   ├── UncoatedFOGRA29.icc
│   ├── WebCoatedFOGRA28.icc
│   ├── WebCoatedSWOP2006Grade3.icc
│   └── WebCoatedSWOP2006Grade5.icc
└── RGB/
    ├── AdobeRGB1998.icc
    ├── AppleRGB.icc
    ├── ColorMatchRGB.icc
    ├── PAL_SECAM.icc
    ├── SMPTE-C.icc
    ├── VideoHD.icc
    ├── VideoNTSC.icc
    └── VideoPAL.icc
```

On Windows, installed profiles are typically found under:
`C:\Windows\System32\spool\drivers\color\`

On macOS:
`/Library/ColorSync/Profiles/`
