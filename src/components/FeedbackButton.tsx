// 反馈入口 (W4.5 D2)
//
// 右上角 🛎 按钮 → 弹"扫码加微信群"二维码 + 邮箱.
// 微信群二维码图 src/assets/feedback-qr.png 占位, 后续替换.

import { useState } from 'react';

const FEEDBACK_QR_SRC = '/src/assets/feedback-qr.png';

export default function FeedbackButton() {
  const [open, setOpen] = useState(false);

  return (
    <>
      <button
        type="button"
        onClick={() => setOpen(true)}
        className="px-3 py-1.5 text-sm bg-surface border border-line rounded-full hover:bg-surface-2 transition-colors"
        aria-label="反馈"
      >
        🛎 反馈
      </button>

      {open && (
        <div
          className="fixed inset-0 z-50 bg-ink/40 flex items-center justify-center p-6"
          onClick={() => setOpen(false)}
        >
          <div
            className="bg-surface rounded-2xl shadow-xl max-w-sm w-full p-6"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="text-center">
              <div className="text-4xl mb-2">💌</div>
              <h2 className="text-lg font-semibold text-ink mb-1">
                遇到问题？有想法？
              </h2>
              <p className="text-sm text-ink-2 mb-4">
                扫码加入用户群，我们会快速回复
              </p>
              <div className="w-48 h-48 mx-auto mb-4 bg-surface-2 border border-line rounded-lg flex items-center justify-center overflow-hidden">
                {/* eslint-disable-next-line @next/next/no-img-element */}
                <img
                  src={FEEDBACK_QR_SRC}
                  alt="用户群二维码"
                  className="w-full h-full object-contain"
                  onError={(e) => {
                    const el = e.currentTarget;
                    el.style.display = 'none';
                    const parent = el.parentElement;
                    if (parent && !parent.querySelector('.qr-fallback')) {
                      const fb = document.createElement('div');
                      fb.className =
                        'qr-fallback text-xs text-ink-2 p-3 text-center';
                      fb.textContent = '二维码图片待补充\n(请微信联系 lihao)';
                      parent.appendChild(fb);
                    }
                  }}
                />
              </div>
              <div className="text-xs text-ink-2 space-y-1">
                <div>
                  或邮件:{' '}
                  <a
                    href="mailto:hello@kidsai.com"
                    className="text-accent-ink underline"
                  >
                    hello@kidsai.com
                  </a>
                </div>
                <div>
                  崩溃日志位置:{' '}
                  <code className="text-[11px] bg-surface-2 px-1 rounded">
                    ~/Library/Logs/KidsAI/
                  </code>
                </div>
              </div>
              <button
                type="button"
                onClick={() => setOpen(false)}
                className="mt-5 px-5 py-2 bg-surface-2 hover:bg-surface-2 rounded-lg text-sm"
              >
                关闭
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}