## 使用
随便进一个b站的 [api](https://api.bilibili.com/x/msgfeed/reply?platform=web&build=0&mobi_app=web) 网页获取 cookie 再输入
- 不加任何参数打开 GUI 界面
- Cli 只有一个功能
    > 删除所有通知：`bilibili-comment-cleaning.exe --remove_notifys "{此处填cookie}"`

## 如何做到的
bilibili 并未公开获取历史所有评论的接口，但是使用消息中心获取被点赞、评论、At 的评论可以获取到大部分历史评论，再进行删除
