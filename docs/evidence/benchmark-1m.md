# 100万资产检索性能基准测试报告 (1M Asset Benchmark)

- **测试日期**: 2026-09-13 04:25:52 UTC
- **脚本入口**: `scripts/benchmark-1m.sh` (实现项 T6-02)
- **验证项**: 解决审计 T6-04（100万资产 p95 < 100ms 实测证据）
- **MeiliSearch 实例**: http://localhost:7700
- **索引资产总数**: 1000000
- **LMDB 数据库体积**: 2335.83 MB
- **并发客户端数**: 50
- **请求总数**: 5000

---

## 1. 压测完整执行日志

```text
=== partisync benchmark ===
Target Docs : 1000000
Start Index : 18
Batch size  : 5000
Concurrency : 50 clients
Requests    : 5000 total
Only Search : false

--- Phase 1: Batch indexing to MeiliSearch ---
Total batches to index: 200 (from doc 18 to 1000000)
Batch 1/200 (docs 18-5018): task UID 537
Batch 2/200 (docs 5018-10018): task UID 538
Batch 3/200 (docs 10018-15018): task UID 539
Batch 4/200 (docs 15018-20018): task UID 540
Batch 5/200 (docs 20018-25018): task UID 541
Batch 6/200 (docs 25018-30018): task UID 542
Batch 7/200 (docs 30018-35018): task UID 543
Batch 8/200 (docs 35018-40018): task UID 544
Batch 9/200 (docs 40018-45018): task UID 545
Batch 10/200 (docs 45018-50018): task UID 546
Batch 11/200 (docs 50018-55018): task UID 547
Batch 12/200 (docs 55018-60018): task UID 548
Batch 13/200 (docs 60018-65018): task UID 549
Batch 14/200 (docs 65018-70018): task UID 550
Batch 15/200 (docs 70018-75018): task UID 551
Batch 16/200 (docs 75018-80018): task UID 552
Batch 17/200 (docs 80018-85018): task UID 553
Batch 18/200 (docs 85018-90018): task UID 554
Batch 19/200 (docs 90018-95018): task UID 555
Batch 20/200 (docs 95018-100018): task UID 556
Batch 21/200 (docs 100018-105018): task UID 557
Batch 22/200 (docs 105018-110018): task UID 558
Batch 23/200 (docs 110018-115018): task UID 559
Batch 24/200 (docs 115018-120018): task UID 560
Batch 25/200 (docs 120018-125018): task UID 561
Batch 26/200 (docs 125018-130018): task UID 562
Batch 27/200 (docs 130018-135018): task UID 563
Batch 28/200 (docs 135018-140018): task UID 564
Batch 29/200 (docs 140018-145018): task UID 565
Batch 30/200 (docs 145018-150018): task UID 566
Batch 31/200 (docs 150018-155018): task UID 567
Batch 32/200 (docs 155018-160018): task UID 568
Batch 33/200 (docs 160018-165018): task UID 569
Batch 34/200 (docs 165018-170018): task UID 570
Batch 35/200 (docs 170018-175018): task UID 571
Batch 36/200 (docs 175018-180018): task UID 572
Batch 37/200 (docs 180018-185018): task UID 573
Batch 38/200 (docs 185018-190018): task UID 574
Batch 39/200 (docs 190018-195018): task UID 575
Batch 40/200 (docs 195018-200018): task UID 576
Batch 41/200 (docs 200018-205018): task UID 577
Batch 42/200 (docs 205018-210018): task UID 578
Batch 43/200 (docs 210018-215018): task UID 579
Batch 44/200 (docs 215018-220018): task UID 580
Batch 45/200 (docs 220018-225018): task UID 581
Batch 46/200 (docs 225018-230018): task UID 582
Batch 47/200 (docs 230018-235018): task UID 583
Batch 48/200 (docs 235018-240018): task UID 584
Batch 49/200 (docs 240018-245018): task UID 585
Batch 50/200 (docs 245018-250018): task UID 586
Batch 51/200 (docs 250018-255018): task UID 587
Batch 52/200 (docs 255018-260018): task UID 588
Batch 53/200 (docs 260018-265018): task UID 589
Batch 54/200 (docs 265018-270018): task UID 590
Batch 55/200 (docs 270018-275018): task UID 591
Batch 56/200 (docs 275018-280018): task UID 592
Batch 57/200 (docs 280018-285018): task UID 593
Batch 58/200 (docs 285018-290018): task UID 594
Batch 59/200 (docs 290018-295018): task UID 595
Batch 60/200 (docs 295018-300018): task UID 596
Batch 61/200 (docs 300018-305018): task UID 597
Batch 62/200 (docs 305018-310018): task UID 598
Batch 63/200 (docs 310018-315018): task UID 599
Batch 64/200 (docs 315018-320018): task UID 600
Batch 65/200 (docs 320018-325018): task UID 601
Batch 66/200 (docs 325018-330018): task UID 602
Batch 67/200 (docs 330018-335018): task UID 603
Batch 68/200 (docs 335018-340018): task UID 604
Batch 69/200 (docs 340018-345018): task UID 605
Batch 70/200 (docs 345018-350018): task UID 606
Batch 71/200 (docs 350018-355018): task UID 607
Batch 72/200 (docs 355018-360018): task UID 608
Batch 73/200 (docs 360018-365018): task UID 609
Batch 74/200 (docs 365018-370018): task UID 610
Batch 75/200 (docs 370018-375018): task UID 611
Batch 76/200 (docs 375018-380018): task UID 612
Batch 77/200 (docs 380018-385018): task UID 613
Batch 78/200 (docs 385018-390018): task UID 614
Batch 79/200 (docs 390018-395018): task UID 615
Batch 80/200 (docs 395018-400018): task UID 616
Batch 81/200 (docs 400018-405018): task UID 617
Batch 82/200 (docs 405018-410018): task UID 618
Batch 83/200 (docs 410018-415018): task UID 619
Batch 84/200 (docs 415018-420018): task UID 620
Batch 85/200 (docs 420018-425018): task UID 621
Batch 86/200 (docs 425018-430018): task UID 622
Batch 87/200 (docs 430018-435018): task UID 623
Batch 88/200 (docs 435018-440018): task UID 624
Batch 89/200 (docs 440018-445018): task UID 625
Batch 90/200 (docs 445018-450018): task UID 626
Batch 91/200 (docs 450018-455018): task UID 627
Batch 92/200 (docs 455018-460018): task UID 628
Batch 93/200 (docs 460018-465018): task UID 629
Batch 94/200 (docs 465018-470018): task UID 630
Batch 95/200 (docs 470018-475018): task UID 631
Batch 96/200 (docs 475018-480018): task UID 632
Batch 97/200 (docs 480018-485018): task UID 633
Batch 98/200 (docs 485018-490018): task UID 634
Batch 99/200 (docs 490018-495018): task UID 635
Batch 100/200 (docs 495018-500018): task UID 636
Batch 101/200 (docs 500018-505018): task UID 637
Batch 102/200 (docs 505018-510018): task UID 638
Batch 103/200 (docs 510018-515018): task UID 639
Batch 104/200 (docs 515018-520018): task UID 640
Batch 105/200 (docs 520018-525018): task UID 641
Batch 106/200 (docs 525018-530018): task UID 642
Batch 107/200 (docs 530018-535018): task UID 643
Batch 108/200 (docs 535018-540018): task UID 644
Batch 109/200 (docs 540018-545018): task UID 645
Batch 110/200 (docs 545018-550018): task UID 646
Batch 111/200 (docs 550018-555018): task UID 647
Batch 112/200 (docs 555018-560018): task UID 648
Batch 113/200 (docs 560018-565018): task UID 649
Batch 114/200 (docs 565018-570018): task UID 650
Batch 115/200 (docs 570018-575018): task UID 651
Batch 116/200 (docs 575018-580018): task UID 652
Batch 117/200 (docs 580018-585018): task UID 653
Batch 118/200 (docs 585018-590018): task UID 654
Batch 119/200 (docs 590018-595018): task UID 655
Batch 120/200 (docs 595018-600018): task UID 656
Batch 121/200 (docs 600018-605018): task UID 657
Batch 122/200 (docs 605018-610018): task UID 658
Batch 123/200 (docs 610018-615018): task UID 659
Batch 124/200 (docs 615018-620018): task UID 660
Batch 125/200 (docs 620018-625018): task UID 661
Batch 126/200 (docs 625018-630018): task UID 662
Batch 127/200 (docs 630018-635018): task UID 663
Batch 128/200 (docs 635018-640018): task UID 664
Batch 129/200 (docs 640018-645018): task UID 665
Batch 130/200 (docs 645018-650018): task UID 666
Batch 131/200 (docs 650018-655018): task UID 667
Batch 132/200 (docs 655018-660018): task UID 668
Batch 133/200 (docs 660018-665018): task UID 669
Batch 134/200 (docs 665018-670018): task UID 670
Batch 135/200 (docs 670018-675018): task UID 671
Batch 136/200 (docs 675018-680018): task UID 672
Batch 137/200 (docs 680018-685018): task UID 673
Batch 138/200 (docs 685018-690018): task UID 674
Batch 139/200 (docs 690018-695018): task UID 675
Batch 140/200 (docs 695018-700018): task UID 676
Batch 141/200 (docs 700018-705018): task UID 677
Batch 142/200 (docs 705018-710018): task UID 678
Batch 143/200 (docs 710018-715018): task UID 679
Batch 144/200 (docs 715018-720018): task UID 680
Batch 145/200 (docs 720018-725018): task UID 681
Batch 146/200 (docs 725018-730018): task UID 682
Batch 147/200 (docs 730018-735018): task UID 683
Batch 148/200 (docs 735018-740018): task UID 684
Batch 149/200 (docs 740018-745018): task UID 685
Batch 150/200 (docs 745018-750018): task UID 686
Batch 151/200 (docs 750018-755018): task UID 687
Batch 152/200 (docs 755018-760018): task UID 688
Batch 153/200 (docs 760018-765018): task UID 689
Batch 154/200 (docs 765018-770018): task UID 690
Batch 155/200 (docs 770018-775018): task UID 691
Batch 156/200 (docs 775018-780018): task UID 692
Batch 157/200 (docs 780018-785018): task UID 693
Batch 158/200 (docs 785018-790018): task UID 694
Batch 159/200 (docs 790018-795018): task UID 695
Batch 160/200 (docs 795018-800018): task UID 696
Batch 161/200 (docs 800018-805018): task UID 697
Batch 162/200 (docs 805018-810018): task UID 698
Batch 163/200 (docs 810018-815018): task UID 699
Batch 164/200 (docs 815018-820018): task UID 700
Batch 165/200 (docs 820018-825018): task UID 701
Batch 166/200 (docs 825018-830018): task UID 702
Batch 167/200 (docs 830018-835018): task UID 703
Batch 168/200 (docs 835018-840018): task UID 704
Batch 169/200 (docs 840018-845018): task UID 705
Batch 170/200 (docs 845018-850018): task UID 706
Batch 171/200 (docs 850018-855018): task UID 707
Batch 172/200 (docs 855018-860018): task UID 708
Batch 173/200 (docs 860018-865018): task UID 709
Batch 174/200 (docs 865018-870018): task UID 710
Batch 175/200 (docs 870018-875018): task UID 711
Batch 176/200 (docs 875018-880018): task UID 712
Batch 177/200 (docs 880018-885018): task UID 713
Batch 178/200 (docs 885018-890018): task UID 714
Batch 179/200 (docs 890018-895018): task UID 715
Batch 180/200 (docs 895018-900018): task UID 716
Batch 181/200 (docs 900018-905018): task UID 717
Batch 182/200 (docs 905018-910018): task UID 718
Batch 183/200 (docs 910018-915018): task UID 719
Batch 184/200 (docs 915018-920018): task UID 720
Batch 185/200 (docs 920018-925018): task UID 721
Batch 186/200 (docs 925018-930018): task UID 722
Batch 187/200 (docs 930018-935018): task UID 723
Batch 188/200 (docs 935018-940018): task UID 724
Batch 189/200 (docs 940018-945018): task UID 725
Batch 190/200 (docs 945018-950018): task UID 726
Batch 191/200 (docs 950018-955018): task UID 727
Batch 192/200 (docs 955018-960018): task UID 728
Batch 193/200 (docs 960018-965018): task UID 729
Batch 194/200 (docs 965018-970018): task UID 730
Batch 195/200 (docs 970018-975018): task UID 731
Batch 196/200 (docs 975018-980018): task UID 732
Batch 197/200 (docs 980018-985018): task UID 733
Batch 198/200 (docs 985018-990018): task UID 734
Batch 199/200 (docs 990018-995018): task UID 735
Batch 200/200 (docs 995018-1000000): task UID 736
Indexing complete
Documents in index: 1000000

--- Phase 2: Search benchmark (50 clients, 5000 requests) ---

=== Results (5000 ok / 0 errors) ===
Duration : 0.86s
RPS      : 5831.77 req/s
p50      : 7.21 ms
p90      : 14.87 ms
p95      : 17.77 ms
p99      : 31.16 ms
min      : 0.81 ms
max      : 48.57 ms
avg      : 8.41 ms

Latency distribution (ms):
      0.8 -     3.2 ms |   669 | ██████████████████████████████
      3.2 -     5.6 ms |  1108 | ██████████████████████████████████████████████████
      5.6 -     8.0 ms |   993 | ████████████████████████████████████████████
      8.0 -    10.4 ms |   815 | ████████████████████████████████████
     10.4 -    12.8 ms |   583 | ██████████████████████████
     12.8 -    15.1 ms |   354 | ███████████████
     15.1 -    17.5 ms |   203 | █████████
     17.5 -    19.9 ms |   122 | █████
     19.9 -    22.3 ms |    52 | ██
     22.3 -    24.7 ms |    30 | █
     24.7 -    27.1 ms |    15 | 
     27.1 -    29.5 ms |     5 | 
     29.5 -    31.9 ms |     2 | 
     31.9 -    34.2 ms |     2 | 
     34.2 -    36.6 ms |     1 | 
     36.6 -    39.0 ms |    30 | █
     39.0 -    41.4 ms |     5 | 
     41.4 -    43.8 ms |     3 | 
     43.8 -    46.2 ms |     3 | 
     46.2 -    48.6 ms |     5 | 

PASS: p95 (17.77 ms) < 100 ms
```

---

## 2. 指标提取与 MCD 达标判定

从基准测试结果中提炼核心指标：
- RPS      : 5831.77 req/s
- p50      : 7.21 ms
- p90      : 14.87 ms
- p95      : 17.77 ms
- p99      : 31.16 ms
- min      : 0.81 ms
- max      : 48.57 ms
- avg      : 8.41 ms
- PASS: p95 (17.77 ms) < 100 ms

- **MCD 目标**: 100 万资产搜索 p95 < 100ms
- **实测判定**: ✅ **通过 (PASS)** — 实测 p95 严格小于 100ms，达成 MCD 工业级性能指标。

