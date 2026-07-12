    const modal      = document.getElementById('modal');
    const modalImg   = document.getElementById('modal-img');
    const modalTitle = document.getElementById('modal-title');
    const modalOpen  = document.getElementById('modal-open');
    const imgArea    = document.querySelector('.modal-img-area');

    const cards = Array.from(document.querySelectorAll('.card'));
    let currentCardIndex = -1;

    // Crop Editor Elements
    const cropBtn              = document.getElementById('modal-crop-btn');
    const cropWorkspace        = document.querySelector('.crop-workspace');
    const viewerFooterControls = document.getElementById('viewer-footer-controls');
    const cropFooterControls   = document.getElementById('crop-footer-controls');
    const cropCanvas           = document.getElementById('crop-canvas');
    const rotateSlider         = document.getElementById('rotate-slider');
    const rotateNum            = document.getElementById('rotate-num');
    const resetCropBtn         = document.getElementById('reset-crop-btn');
    const cropOverwrite        = document.getElementById('crop-overwrite');
    const cropFilenameGroup    = document.getElementById('crop-filename-group');
    const cropFilename         = document.getElementById('crop-filename');
    const cropCancelBtn        = document.getElementById('crop-cancel-btn');
    const cropSaveBtn          = document.getElementById('crop-save-btn');

    let cropImg = new Image();
    let cropRotation = 0;
    let cropBox = null;
    const cropPad = 50;
    let cropDragState = { active: false, type: null, handle: null, startX: 0, startY: 0, boxStartX: 0, boxStartY: 0, boxStartW: 0, boxStartH: 0 };

    function updateModalContent() {
      if (currentCardIndex < 0 || currentCardIndex >= cards.length) return;
      const card = cards[currentCardIndex];
      const path   = card.dataset.path;
      const dir    = card.dataset.dir;
      const imgSrc = card.dataset.imgSrc;

      modalImg.src           = imgSrc;
      modalImg.alt           = path;
      modalTitle.textContent = path;
      modalTitle.href        = '/explore?dir=' + encodeURIComponent(dir);
      modalOpen.href         = imgSrc;
    }

    function showNext() {
      if (cards.length === 0) return;
      currentCardIndex = (currentCardIndex + 1) % cards.length;
      updateModalContent();
    }

    function showPrev() {
      if (cards.length === 0) return;
      currentCardIndex = (currentCardIndex - 1 + cards.length) % cards.length;
      updateModalContent();
    }

    function openModal(card) {
      currentCardIndex = cards.indexOf(card);
      updateModalContent();
      if (!modal.classList.contains('open')) {
        modal.classList.add('open');
        history.pushState({ modal: true }, '');
      }
    }

    function exitCropMode() {
      modal.classList.remove('crop-mode');
      viewerFooterControls.style.display = 'flex';
      cropFooterControls.style.display = 'none';
    }

    function closeModal(fromPopstate) {
      exitCropMode();
      modal.classList.remove('open');
      modalImg.src = '';
      currentCardIndex = -1;
      if (!fromPopstate) history.back();
    }

    window.addEventListener('popstate', () => {
      if (modal.classList.contains('open')) closeModal(true);
    });

    document.querySelectorAll('.card').forEach(card => {
      const cb = card.querySelector('.card-checkbox');
      if (cb) {
        cb.addEventListener('click', e => {
          e.stopPropagation();
          toggleCardSelection(card, cb.checked);
        });
      }

      card.addEventListener('click', e => {
        if (e.shiftKey) {
          e.preventDefault();
          e.stopPropagation();
          handleRangeSelection(card);
          return;
        }

        if (e.target.closest('.card-checkbox-container')) {
          return;
        }

        openModal(card);
      });
    });

    document.getElementById('modal-close').addEventListener('click', () => closeModal(false));

    modal.addEventListener('click', e => {
      if (e.target === modal) closeModal(false);
    });

    document.addEventListener('keydown', e => {
      if (!modal.classList.contains('open')) return;
      if (e.key === 'Escape') {
        closeModal(false);
      } else if (e.key === 'ArrowRight') {
        if (modal.classList.contains('crop-mode')) return;
        showNext();
      } else if (e.key === 'ArrowLeft') {
        if (modal.classList.contains('crop-mode')) return;
        showPrev();
      }
    });

    imgArea.addEventListener('mousemove', e => {
      if (modal.classList.contains('crop-mode')) return;
      const rect = imgArea.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const width = rect.width;
      if (x < width * 0.25) {
        imgArea.style.cursor = 'w-resize';
      } else if (x > width * 0.75) {
        imgArea.style.cursor = 'e-resize';
      } else {
        imgArea.style.cursor = 'default';
      }
    });

    imgArea.addEventListener('click', e => {
      if (modal.classList.contains('crop-mode')) return;
      const rect = imgArea.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const width = rect.width;
      if (x < width * 0.25) {
        showPrev();
      } else if (x > width * 0.75) {
        showNext();
      }
    });

    // ── Crop Editor interactive logic ─────────────────────────────────────────

    function initCropEditor() {
      cropRotation = 0;
      rotateSlider.value = 0;
      rotateNum.value = 0;
      cropBox = null;
      
      const card = cards[currentCardIndex];
      const path = card.dataset.path;
      
      cropOverwrite.checked = true;
      cropFilenameGroup.style.display = 'none';
      
      const filename = path.split('/').pop();
      const dotIndex = filename.lastIndexOf('.');
      const stem = dotIndex !== -1 ? filename.substring(0, dotIndex) : filename;
      const ext = dotIndex !== -1 ? filename.substring(dotIndex + 1) : 'png';
      cropFilename.value = stem + '_cropped.' + ext;
      
      cropImg.src = modalImg.src;
      cropImg.onload = () => {
        // Wait for DOM layout to settle and size computations to be correct
        setTimeout(drawCropCanvas, 150);
      };
    }

    function drawCropCanvas() {
      if (!cropImg.complete || cropImg.naturalWidth === 0) return;
      
      const ctx = cropCanvas.getContext('2d');
      const w_orig = cropImg.naturalWidth;
      const h_orig = cropImg.naturalHeight;
      
      const alpha = cropRotation * Math.PI / 180.0;
      const cos = Math.abs(Math.cos(alpha));
      const sin = Math.abs(Math.sin(alpha));
      const w_rot = w_orig * cos + h_orig * sin;
      const h_rot = w_orig * sin + h_orig * cos;
      
      const w_work = w_rot + cropPad * 2;
      const h_work = h_rot + cropPad * 2;
      
      const container = document.querySelector('.crop-canvas-container');
      const rect = container.getBoundingClientRect();
      const w_max = Math.max(10, rect.width - 48) || 600;
      const h_max = Math.max(10, rect.height - 48) || 400;
      
      const scale = Math.min(w_max / w_work, h_max / h_work, 1.0);
      
      cropCanvas.width = w_work * scale;
      cropCanvas.height = h_work * scale;
      
      const imgScale = scale;
      const cx_canvas = cropCanvas.width / 2;
      const cy_canvas = cropCanvas.height / 2;
      
      if (!cropBox) {
        const bw = w_rot * imgScale * 0.8;
        const bh = h_rot * imgScale * 0.8;
        cropBox = {
          x: cx_canvas - bw / 2,
          y: cy_canvas - bh / 2,
          w: bw,
          h: bh
        };
      }
      
      // Checkerboard background
      ctx.fillStyle = '#1e2230';
      ctx.fillRect(0, 0, cropCanvas.width, cropCanvas.height);
      ctx.fillStyle = '#151821';
      const chSize = 12;
      for (let y = 0; y < cropCanvas.height; y += chSize * 2) {
        for (let x = 0; x < cropCanvas.width; x += chSize * 2) {
          ctx.fillRect(x, y, chSize, chSize);
          ctx.fillRect(x + chSize, y + chSize, chSize, chSize);
        }
      }
      
      // Draw rotated image
      ctx.save();
      ctx.translate(cx_canvas, cy_canvas);
      ctx.rotate(alpha);
      ctx.drawImage(cropImg, -w_orig * imgScale / 2, -h_orig * imgScale / 2, w_orig * imgScale, h_orig * imgScale);
      ctx.restore();
      
      // Dark overlay
      ctx.fillStyle = 'rgba(0, 0, 0, 0.6)';
      ctx.fillRect(0, 0, cropCanvas.width, cropCanvas.height);
      
      // Redraw inside crop box
      ctx.save();
      ctx.beginPath();
      ctx.rect(cropBox.x, cropBox.y, cropBox.w, cropBox.h);
      ctx.clip();
      ctx.translate(cx_canvas, cy_canvas);
      ctx.rotate(alpha);
      ctx.drawImage(cropImg, -w_orig * imgScale / 2, -h_orig * imgScale / 2, w_orig * imgScale, h_orig * imgScale);
      ctx.restore();
      
      // Crop border
      ctx.strokeStyle = '#7c6af7';
      ctx.lineWidth = 2;
      ctx.strokeRect(cropBox.x, cropBox.y, cropBox.w, cropBox.h);
      
      // Dashed grids
      ctx.strokeStyle = 'rgba(255, 255, 255, 0.4)';
      ctx.lineWidth = 1;
      ctx.setLineDash([4, 4]);
      ctx.beginPath();
      ctx.moveTo(cropBox.x + cropBox.w / 3, cropBox.y);
      ctx.lineTo(cropBox.x + cropBox.w / 3, cropBox.y + cropBox.h);
      ctx.moveTo(cropBox.x + 2 * cropBox.w / 3, cropBox.y);
      ctx.lineTo(cropBox.x + 2 * cropBox.w / 3, cropBox.y + cropBox.h);
      ctx.moveTo(cropBox.x, cropBox.y + cropBox.h / 3);
      ctx.lineTo(cropBox.x + cropBox.w, cropBox.y + cropBox.h / 3);
      ctx.moveTo(cropBox.x, cropBox.y + 2 * cropBox.h / 3);
      ctx.lineTo(cropBox.x + cropBox.w, cropBox.y + 2 * cropBox.h / 3);
      ctx.stroke();
      ctx.setLineDash([]);
      
      // Handles
      const handleSize = 8;
      ctx.fillStyle = '#7c6af7';
      ctx.strokeStyle = '#fff';
      ctx.lineWidth = 1.5;
      
      const handles = {
        tl: { x: cropBox.x, y: cropBox.y },
        tr: { x: cropBox.x + cropBox.w, y: cropBox.y },
        bl: { x: cropBox.x, y: cropBox.y + cropBox.h },
        br: { x: cropBox.x + cropBox.w, y: cropBox.y + cropBox.h }
      };
      
      for (const key in handles) {
        const h = handles[key];
        ctx.fillRect(h.x - handleSize / 2, h.y - handleSize / 2, handleSize, handleSize);
        ctx.strokeRect(h.x - handleSize / 2, h.y - handleSize / 2, handleSize, handleSize);
      }
    }

    function applySnapping(type, handle) {
      if (!cropImg.complete || cropImg.naturalWidth === 0 || !cropBox) return;
      
      const w_orig = cropImg.naturalWidth;
      const h_orig = cropImg.naturalHeight;
      const alpha = cropRotation * Math.PI / 180.0;
      const cos = Math.abs(Math.cos(alpha));
      const sin = Math.abs(Math.sin(alpha));
      const w_rot = w_orig * cos + h_orig * sin;
      const h_rot = w_orig * sin + h_orig * cos;
      
      const w_work = w_rot + cropPad * 2;
      
      const scale = cropCanvas.width / w_work;
      const imgScale = scale;
      const cx_canvas = cropCanvas.width / 2;
      const cy_canvas = cropCanvas.height / 2;
      
      const rx = cx_canvas - (w_rot * imgScale) / 2;
      const ry = cy_canvas - (h_rot * imgScale) / 2;
      const rx_end = rx + w_rot * imgScale;
      const ry_end = ry + h_rot * imgScale;
      
      const snapThresh = 8;
      
      if (type === 'move') {
        if (Math.abs(cropBox.x - rx) < snapThresh) {
          cropBox.x = rx;
        } else if (Math.abs(cropBox.x + cropBox.w - rx_end) < snapThresh) {
          cropBox.x = rx_end - cropBox.w;
        }
        if (Math.abs(cropBox.y - ry) < snapThresh) {
          cropBox.y = ry;
        } else if (Math.abs(cropBox.y + cropBox.h - ry_end) < snapThresh) {
          cropBox.y = ry_end - cropBox.h;
        }
      } else if (type === 'resize') {
        if (handle === 'br') {
          if (Math.abs(cropBox.x + cropBox.w - rx_end) < snapThresh) {
            cropBox.w = rx_end - cropBox.x;
          }
          if (Math.abs(cropBox.y + cropBox.h - ry_end) < snapThresh) {
            cropBox.h = ry_end - cropBox.y;
          }
        } else if (handle === 'tl') {
          const oppositeX = cropBox.x + cropBox.w;
          const oppositeY = cropBox.y + cropBox.h;
          if (Math.abs(cropBox.x - rx) < snapThresh) {
            cropBox.x = rx;
            cropBox.w = oppositeX - rx;
          }
          if (Math.abs(cropBox.y - ry) < snapThresh) {
            cropBox.y = ry;
            cropBox.h = oppositeY - ry;
          }
        } else if (handle === 'tr') {
          const oppositeY = cropBox.y + cropBox.h;
          if (Math.abs(cropBox.x + cropBox.w - rx_end) < snapThresh) {
            cropBox.w = rx_end - cropBox.x;
          }
          if (Math.abs(cropBox.y - ry) < snapThresh) {
            cropBox.y = ry;
            cropBox.h = oppositeY - ry;
          }
        } else if (handle === 'bl') {
          const oppositeX = cropBox.x + cropBox.w;
          if (Math.abs(cropBox.x - rx) < snapThresh) {
            cropBox.x = rx;
            cropBox.w = oppositeX - rx;
          }
          if (Math.abs(cropBox.y + cropBox.h - ry_end) < snapThresh) {
            cropBox.h = ry_end - cropBox.y;
          }
        }
      } else if (type === 'draw') {
        if (Math.abs(cropBox.x - rx) < snapThresh) {
          cropBox.w = cropBox.w + (cropBox.x - rx);
          cropBox.x = rx;
        }
        if (Math.abs(cropBox.y - ry) < snapThresh) {
          cropBox.h = cropBox.h + (cropBox.y - ry);
          cropBox.y = ry;
        }
        if (Math.abs(cropBox.x + cropBox.w - rx_end) < snapThresh) {
          cropBox.w = rx_end - cropBox.x;
        }
        if (Math.abs(cropBox.y + cropBox.h - ry_end) < snapThresh) {
          cropBox.h = ry_end - cropBox.y;
        }
      }
    }

    function getMousePos(e) {
      const rect = cropCanvas.getBoundingClientRect();
      return {
        x: (e.clientX - rect.left) * (cropCanvas.width / rect.width),
        y: (e.clientY - rect.top) * (cropCanvas.height / rect.height)
      };
    }
    
    function getHandleAt(mx, my) {
      const handleSize = 12;
      const handles = {
        tl: { x: cropBox.x, y: cropBox.y },
        tr: { x: cropBox.x + cropBox.w, y: cropBox.y },
        bl: { x: cropBox.x, y: cropBox.y + cropBox.h },
        br: { x: cropBox.x + cropBox.w, y: cropBox.y + cropBox.h }
      };
      for (const key in handles) {
        const h = handles[key];
        if (Math.abs(mx - h.x) <= handleSize && Math.abs(my - h.y) <= handleSize) {
          return key;
        }
      }
      return null;
    }
    
    function isInsideBox(mx, my) {
      return mx >= cropBox.x && mx <= cropBox.x + cropBox.w &&
             my >= cropBox.y && my <= cropBox.y + cropBox.h;
    }
    
    cropCanvas.addEventListener('mousedown', e => {
      e.preventDefault();
      const { x, y } = getMousePos(e);
      const handle = getHandleAt(x, y);
      
      if (handle) {
        cropDragState = {
          active: true,
          type: 'resize',
          handle: handle,
          startX: x,
          startY: y,
          boxStartX: cropBox.x,
          boxStartY: cropBox.y,
          boxStartW: cropBox.w,
          boxStartH: cropBox.h
        };
      } else if (isInsideBox(x, y)) {
        cropDragState = {
          active: true,
          type: 'move',
          startX: x,
          startY: y,
          boxStartX: cropBox.x,
          boxStartY: cropBox.y,
          boxStartW: cropBox.w,
          boxStartH: cropBox.h
        };
      } else {
        cropDragState = {
          active: true,
          type: 'draw',
          startX: x,
          startY: y
        };
        cropBox = { x: x, y: y, w: 1, h: 1 };
      }
    });
    
    cropCanvas.addEventListener('mousemove', e => {
      const { x, y } = getMousePos(e);
      
      if (!cropDragState.active) {
        const handle = getHandleAt(x, y);
        if (handle === 'tl' || handle === 'br') {
          cropCanvas.style.cursor = 'nwse-resize';
        } else if (handle === 'tr' || handle === 'bl') {
          cropCanvas.style.cursor = 'nesw-resize';
        } else if (isInsideBox(x, y)) {
          cropCanvas.style.cursor = 'move';
        } else {
          cropCanvas.style.cursor = 'crosshair';
        }
      }
      
      if (cropDragState.active) {
        if (cropDragState.type === 'move') {
          const dx = x - cropDragState.startX;
          const dy = y - cropDragState.startY;
          cropBox.x = Math.max(0, Math.min(cropCanvas.width - cropBox.w, cropDragState.boxStartX + dx));
          cropBox.y = Math.max(0, Math.min(cropCanvas.height - cropBox.h, cropDragState.boxStartY + dy));
        } else if (cropDragState.type === 'resize') {
          const dx = x - cropDragState.startX;
          const dy = y - cropDragState.startY;
          
          if (cropDragState.handle === 'br') {
            cropBox.w = Math.max(10, Math.min(cropCanvas.width - cropDragState.boxStartX, cropDragState.boxStartW + dx));
            cropBox.h = Math.max(10, Math.min(cropCanvas.height - cropDragState.boxStartY, cropDragState.boxStartH + dy));
          } else if (cropDragState.handle === 'tl') {
            const oppositeX = cropDragState.boxStartX + cropDragState.boxStartW;
            const oppositeY = cropDragState.boxStartY + cropDragState.boxStartH;
            cropBox.x = Math.max(0, Math.min(oppositeX - 10, cropDragState.boxStartX + dx));
            cropBox.y = Math.max(0, Math.min(oppositeY - 10, cropDragState.boxStartY + dy));
            cropBox.w = oppositeX - cropBox.x;
            cropBox.h = oppositeY - cropBox.y;
          } else if (cropDragState.handle === 'tr') {
            const oppositeY = cropDragState.boxStartY + cropDragState.boxStartH;
            cropBox.w = Math.max(10, Math.min(cropCanvas.width - cropDragState.boxStartX, cropDragState.boxStartW + dx));
            cropBox.y = Math.max(0, Math.min(oppositeY - 10, cropDragState.boxStartY + dy));
            cropBox.h = oppositeY - cropBox.y;
          } else if (cropDragState.handle === 'bl') {
            const oppositeX = cropDragState.boxStartX + cropDragState.boxStartW;
            cropBox.x = Math.max(0, Math.min(oppositeX - 10, cropDragState.boxStartX + dx));
            cropBox.w = oppositeX - cropBox.x;
            cropBox.h = Math.max(10, Math.min(cropCanvas.height - cropDragState.boxStartY, cropDragState.boxStartH + dy));
          }
        } else if (cropDragState.type === 'draw') {
          const x0 = Math.min(cropDragState.startX, x);
          const y0 = Math.min(cropDragState.startY, y);
          const w0 = Math.abs(x - cropDragState.startX);
          const h0 = Math.abs(y - cropDragState.startY);
          cropBox = {
            x: Math.max(0, x0),
            y: Math.max(0, y0),
            w: Math.max(10, Math.min(cropCanvas.width - x0, w0)),
            h: Math.max(10, Math.min(cropCanvas.height - y0, h0))
          };
        }
        applySnapping(cropDragState.type, cropDragState.handle);
        drawCropCanvas();
      }
    });
    
    window.addEventListener('mouseup', () => {
      cropDragState.active = false;
    });

    rotateSlider.addEventListener('input', e => {
      cropRotation = parseFloat(e.target.value);
      rotateNum.value = cropRotation;
      drawCropCanvas();
    });
    
    rotateNum.addEventListener('input', e => {
      let val = parseFloat(e.target.value);
      if (isNaN(val)) val = 0;
      val = Math.max(-180, Math.min(180, val));
      cropRotation = val;
      rotateSlider.value = val;
      drawCropCanvas();
    });
    
    resetCropBtn.addEventListener('click', () => {
      cropRotation = 0;
      rotateSlider.value = 0;
      rotateNum.value = 0;
      cropBox = null;
      drawCropCanvas();
    });

    cropOverwrite.addEventListener('change', () => {
      if (cropOverwrite.checked) {
        cropFilenameGroup.style.display = 'none';
      } else {
        cropFilenameGroup.style.display = 'flex';
      }
    });

    cropBtn.addEventListener('click', () => {
      modal.classList.add('crop-mode');
      viewerFooterControls.style.display = 'none';
      cropFooterControls.style.display = 'flex';
      initCropEditor();
    });
    
    cropCancelBtn.addEventListener('click', exitCropMode);

    cropSaveBtn.addEventListener('click', () => {
      if (!cropImg.complete || cropImg.naturalWidth === 0) return;
      
      const card = cards[currentCardIndex];
      const path = card.dataset.path;
      
      const w_orig = cropImg.naturalWidth;
      const h_orig = cropImg.naturalHeight;
      const alpha = cropRotation * Math.PI / 180.0;
      const cos = Math.abs(Math.cos(alpha));
      const sin = Math.abs(Math.sin(alpha));
      const w_rot = w_orig * cos + h_orig * sin;
      
      const w_work = w_rot + cropPad * 2;
      const scale = cropCanvas.width / w_work;
      
      const x_rot = cropBox.x / scale - cropPad;
      const y_rot = cropBox.y / scale - cropPad;
      const w_rot_out = cropBox.w / scale;
      const h_rot_out = cropBox.h / scale;
      
      const finalX = Math.round(x_rot);
      const finalY = Math.round(y_rot);
      const finalW = Math.round(w_rot_out);
      const finalH = Math.round(h_rot_out);
      
      const overwrite = cropOverwrite.checked;
      const newFilename = overwrite ? null : cropFilename.value;
      
      const payload = {
        input_path: path,
        overwrite: overwrite,
        new_filename: newFilename,
        rotate_deg: cropRotation,
        crop_rect: {
          x: finalX,
          y: finalY,
          w: finalW,
          h: finalH
        }
      };
      
      cropSaveBtn.disabled = true;
      cropSaveBtn.textContent = 'Saving...';
      
      fetch('/api/crop', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json'
        },
        body: JSON.stringify(payload)
      })
      .then(res => res.json())
      .then(data => {
        cropSaveBtn.disabled = false;
        cropSaveBtn.textContent = 'Save Crop';
        if (data.success) {
          exitCropMode();
          location.reload();
        } else {
          alert('Failed to crop image: ' + data.message);
        }
      })
      .catch(err => {
        cropSaveBtn.disabled = false;
        cropSaveBtn.textContent = 'Save Crop';
        alert('Error saving crop: ' + err);
      });
    });

    // ── Multi-file Selection & Actions ─────────────────────────────────────────
    let lastSelectedCard = null;

    function toggleCardSelection(card, isSelected) {
      const cb = card.querySelector('.card-checkbox');
      if (cb) cb.checked = isSelected;
      if (isSelected) {
        card.classList.add('selected');
      } else {
        card.classList.remove('selected');
      }
      lastSelectedCard = card;
      updateSelectionBar();
    }

    function handleRangeSelection(targetCard) {
      if (!lastSelectedCard || lastSelectedCard === targetCard) {
        const cb = targetCard.querySelector('.card-checkbox');
        if (cb) {
          toggleCardSelection(targetCard, !cb.checked);
        }
        return;
      }

      const idxStart = cards.indexOf(lastSelectedCard);
      const idxEnd = cards.indexOf(targetCard);
      if (idxStart === -1 || idxEnd === -1) return;

      const min = Math.min(idxStart, idxEnd);
      const max = Math.max(idxStart, idxEnd);

      const targetCb = targetCard.querySelector('.card-checkbox');
      const targetState = targetCb ? !targetCb.checked : true;

      for (let i = min; i <= max; i++) {
        toggleCardSelection(cards[i], targetState);
      }
      lastSelectedCard = targetCard;
    }

    const selectionBar = document.getElementById('selection-bar');
    const selectionCount = document.getElementById('selection-count');
    const selectAllBtn = document.getElementById('select-all-btn');
    const deselectAllBtn = document.getElementById('deselect-all-btn');
    const moveBtn = document.getElementById('move-btn');
    const deleteBtn = document.getElementById('delete-btn');

    function updateSelectionBar() {
      const selectedCards = cards.filter(c => c.classList.contains('selected'));
      const count = selectedCards.length;
      if (count > 0) {
        selectionCount.textContent = `${count} image${count === 1 ? '' : 's'} selected`;
        selectionBar.classList.add('active');
      } else {
        selectionBar.classList.remove('active');
      }
    }

    if (selectAllBtn) {
      selectAllBtn.addEventListener('click', () => {
        cards.forEach(c => toggleCardSelection(c, true));
      });
    }

    if (deselectAllBtn) {
      deselectAllBtn.addEventListener('click', () => {
        cards.forEach(c => toggleCardSelection(c, false));
      });
    }

    // Move Modal Elements & Logic
    const moveModal = document.getElementById('move-modal');
    const moveModalClose = document.getElementById('move-modal-close');
    const moveCancelBtn = document.getElementById('move-cancel-btn');
    const moveConfirmBtn = document.getElementById('move-confirm-btn');
    const moveDestPath = document.getElementById('move-dest-path');
    const moveAutoCreate = document.getElementById('move-auto-create');
    const moveCollision = document.getElementById('move-collision');
    const moveError = document.getElementById('move-error');

    function openMoveModal() {
      const urlParams = new URLSearchParams(window.location.search);
      const currentDir = urlParams.get('dir') || '';
      moveDestPath.value = currentDir;
      
      moveError.style.display = 'none';
      moveError.textContent = '';
      moveModal.classList.add('open');
    }

    function closeMoveModal() {
      moveModal.classList.remove('open');
    }

    if (moveBtn) {
      moveBtn.addEventListener('click', openMoveModal);
    }
    if (moveModalClose) {
      moveModalClose.addEventListener('click', closeMoveModal);
    }
    if (moveCancelBtn) {
      moveCancelBtn.addEventListener('click', closeMoveModal);
    }

    moveModal.addEventListener('click', e => {
      if (e.target === moveModal) closeMoveModal();
    });

    if (moveConfirmBtn) {
      moveConfirmBtn.addEventListener('click', async () => {
        const selectedCards = cards.filter(c => c.classList.contains('selected'));
        const count = selectedCards.length;
        if (count === 0) return;

        const dest = moveDestPath.value.trim();
        if (!dest) {
          moveError.textContent = 'Please enter a destination directory path.';
          moveError.style.display = 'block';
          return;
        }

        moveError.style.display = 'none';
        moveConfirmBtn.disabled = true;
        moveConfirmBtn.textContent = 'Moving...';

        const paths = selectedCards.map(c => c.dataset.path);

        try {
          const resp = await fetch('/api/explore/move', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
              paths,
              destination_dir: dest,
              auto_create: moveAutoCreate.checked,
              collision_behavior: moveCollision.value
            })
          });
          
          const result = await resp.json();
          if (result.success) {
            alert(result.message);
            closeMoveModal();
            window.location.reload();
          } else {
            moveError.textContent = result.message;
            moveError.style.display = 'block';
          }
        } catch (err) {
          moveError.textContent = `Request failed: ${err}`;
          moveError.style.display = 'block';
        } finally {
          moveConfirmBtn.disabled = false;
          moveConfirmBtn.textContent = 'Move Files';
        }
      });
    }

    if (deleteBtn) {
      deleteBtn.addEventListener('click', async () => {
        const selectedCards = cards.filter(c => c.classList.contains('selected'));
        const count = selectedCards.length;
        if (count === 0) return;

        const confirmMsg = `Are you sure you want to permanently delete the ${count} selected file(s) from disk and database?`;
        if (!confirm(confirmMsg)) return;

        const paths = selectedCards.map(c => c.dataset.path);
        
        try {
          const resp = await fetch('/api/explore/delete', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ paths })
          });
          const result = await resp.json();
          if (result.success) {
            alert(result.message);
            window.location.reload();
          } else {
            alert(`Error: ${result.message}`);
          }
        } catch (err) {
          alert(`Failed to delete: ${err}`);
        }
      });
    }
