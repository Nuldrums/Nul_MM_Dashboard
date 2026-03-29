import { useState, useRef, useEffect } from 'react';
import { ChevronDown, Plus, Edit2, Trash2, User, Check, X } from 'lucide-react';
import { useProfiles, useCreateProfile, useUpdateProfile, useDeleteProfile } from '../hooks/useProfiles';
import { useActiveProfile } from '../hooks/useActiveProfile';

const PRESET_COLORS = [
  '#6366f1', // indigo
  '#8b5cf6', // violet
  '#ec4899', // pink
  '#ef4444', // red
  '#f97316', // orange
  '#eab308', // yellow
  '#22c55e', // green
  '#06b6d4', // cyan
  '#3b82f6', // blue
  '#64748b', // slate
];

export default function ProfileSelector() {
  const { data: profiles } = useProfiles();
  const { activeProfileId, setActiveProfileId, activeProfile } = useActiveProfile();
  const createProfile = useCreateProfile();
  const updateProfile = useUpdateProfile();
  const deleteProfile = useDeleteProfile();

  const [open, setOpen] = useState(false);
  const [showCreateForm, setShowCreateForm] = useState(false);
  const [newName, setNewName] = useState('');
  const [newColor, setNewColor] = useState(PRESET_COLORS[0]);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editName, setEditName] = useState('');
  const [editColor, setEditColor] = useState('');

  const dropdownRef = useRef<HTMLDivElement>(null);

  // Close dropdown on outside click
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setOpen(false);
        setShowCreateForm(false);
        setEditingId(null);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, []);

  const handleCreate = () => {
    if (!newName.trim()) return;
    createProfile.mutate(
      { name: newName.trim(), avatar_color: newColor },
      {
        onSuccess: (profile) => {
          setActiveProfileId(profile.id);
          setNewName('');
          setNewColor(PRESET_COLORS[0]);
          setShowCreateForm(false);
        },
      }
    );
  };

  const handleStartEdit = (id: string, name: string, color: string) => {
    setEditingId(id);
    setEditName(name);
    setEditColor(color);
  };

  const handleSaveEdit = () => {
    if (!editingId || !editName.trim()) return;
    updateProfile.mutate(
      { id: editingId, name: editName.trim(), avatar_color: editColor },
      { onSuccess: () => setEditingId(null) }
    );
  };

  const handleDelete = (id: string) => {
    deleteProfile.mutate(id, {
      onSuccess: () => {
        if (activeProfileId === id) {
          const remaining = profiles?.filter((p) => p.id !== id);
          setActiveProfileId(remaining && remaining.length > 0 ? remaining[0].id : null);
        }
      },
    });
  };

  const handleSelect = (id: string) => {
    setActiveProfileId(id);
    setOpen(false);
    setShowCreateForm(false);
    setEditingId(null);
  };

  return (
    <div className="profile-selector" ref={dropdownRef}>
      <button
        className="profile-current"
        onClick={() => {
          setOpen(!open);
          if (open) {
            setShowCreateForm(false);
            setEditingId(null);
          }
        }}
      >
        {activeProfile ? (
          <>
            <span
              className="profile-dot"
              style={{ background: activeProfile.avatar_color }}
            />
            <span className="profile-current-name">{activeProfile.name}</span>
          </>
        ) : (
          <>
            <User size={14} />
            <span className="profile-current-name">Select Profile</span>
          </>
        )}
        <ChevronDown size={14} className={`profile-chevron ${open ? 'profile-chevron-open' : ''}`} />
      </button>

      {open && (
        <div className="profile-dropdown">
          {/* Profile list */}
          {profiles && profiles.length > 0 && (
            <div className="profile-list">
              {profiles.map((profile) =>
                editingId === profile.id ? (
                  <div key={profile.id} className="profile-edit-row">
                    <input
                      className="profile-edit-input"
                      value={editName}
                      onChange={(e) => setEditName(e.target.value)}
                      autoFocus
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') handleSaveEdit();
                        if (e.key === 'Escape') setEditingId(null);
                      }}
                    />
                    <div className="profile-color-swatches profile-color-swatches-sm">
                      {PRESET_COLORS.map((c) => (
                        <button
                          key={c}
                          type="button"
                          className={`profile-color-swatch ${editColor === c ? 'selected' : ''}`}
                          style={{ background: c }}
                          onClick={() => setEditColor(c)}
                        />
                      ))}
                    </div>
                    <div className="profile-edit-actions">
                      <button className="profile-icon-btn profile-icon-btn-confirm" onClick={handleSaveEdit} title="Save">
                        <Check size={13} />
                      </button>
                      <button className="profile-icon-btn" onClick={() => setEditingId(null)} title="Cancel">
                        <X size={13} />
                      </button>
                    </div>
                  </div>
                ) : (
                  <div
                    key={profile.id}
                    className={`profile-option ${activeProfileId === profile.id ? 'profile-option-active' : ''}`}
                  >
                    <button
                      className="profile-option-main"
                      onClick={() => handleSelect(profile.id)}
                    >
                      <span
                        className="profile-dot"
                        style={{ background: profile.avatar_color }}
                      />
                      <span className="profile-option-name">{profile.name}</span>
                    </button>
                    <div className="profile-option-actions">
                      <button
                        className="profile-icon-btn"
                        onClick={(e) => {
                          e.stopPropagation();
                          handleStartEdit(profile.id, profile.name, profile.avatar_color);
                        }}
                        title="Edit profile"
                      >
                        <Edit2 size={12} />
                      </button>
                      <button
                        className="profile-icon-btn profile-icon-btn-danger"
                        onClick={(e) => {
                          e.stopPropagation();
                          handleDelete(profile.id);
                        }}
                        title="Delete profile"
                      >
                        <Trash2 size={12} />
                      </button>
                    </div>
                  </div>
                )
              )}
            </div>
          )}

          {/* Create new profile */}
          {showCreateForm ? (
            <div className="profile-create-form">
              <input
                className="profile-create-input"
                type="text"
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
                placeholder="Profile name"
                autoFocus
                onKeyDown={(e) => {
                  if (e.key === 'Enter') handleCreate();
                  if (e.key === 'Escape') setShowCreateForm(false);
                }}
              />
              <div className="profile-color-swatches">
                {PRESET_COLORS.map((c) => (
                  <button
                    key={c}
                    type="button"
                    className={`profile-color-swatch ${newColor === c ? 'selected' : ''}`}
                    style={{ background: c }}
                    onClick={() => setNewColor(c)}
                  />
                ))}
              </div>
              <div className="profile-create-actions">
                <button
                  className="btn btn-primary btn-sm"
                  onClick={handleCreate}
                  disabled={createProfile.isPending || !newName.trim()}
                >
                  {createProfile.isPending ? 'Creating...' : 'Create'}
                </button>
                <button
                  className="btn btn-ghost btn-sm"
                  onClick={() => {
                    setShowCreateForm(false);
                    setNewName('');
                  }}
                >
                  Cancel
                </button>
              </div>
            </div>
          ) : (
            <button
              className="profile-add-btn"
              onClick={() => setShowCreateForm(true)}
            >
              <Plus size={14} />
              Create new profile
            </button>
          )}
        </div>
      )}
    </div>
  );
}
